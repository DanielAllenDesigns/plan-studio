//! Plan drawing: grid, rooms, walls (mitered outlines from
//! `plan_core::joins`), openings, dimensions, CAD items, selection and hover
//! highlights. Layer visibility comes from `project.layers`. Tools draw their
//! own overlays on top (`Tool::draw_overlay`).

use super::selection::ObjectRef;
use super::snap::{SnapKind, SnapResult};
use super::{Camera, EditorContext};
use crate::theme::Palette;
use crate::toolbar::ViewFlag;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::{Dimension, DimensionKind, Opening, OpeningKind, Wall, WallKind};

/// Everything under the tool overlay.
pub fn draw_plan(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let pal = &cx.palette;
    painter.rect_filled(cam.rect, 0.0, pal.background);
    draw_grid(cx, painter, cam);
    crate::editor::site_view::draw_site(cx, painter, cam);
    draw_rooms(cx, painter, cam);
    crate::editor::stairs_view::draw_stairs(cx, painter, cam);
    crate::editor::placed::draw_placed(cx, painter, cam);
    crate::editor::roof_view::draw_roofs(cx, painter, cam);
    draw_walls(cx, painter, cam);
    let floor = cx.floor();
    for wall in &floor.walls {
        for o in floor.openings_on(wall.id) {
            if cx.layers().is_visible(opening_layer(o)) {
                draw_opening(painter, cam, wall, o, pal, false);
            }
        }
    }
    // Devices sit on the wall faces: over the wall fill and the openings.
    crate::editor::site_view::draw_devices(cx, painter, cam);
    let fmt = cx.defaults.dim_format();
    for d in &floor.dimensions {
        let layer = match d.kind {
            DimensionKind::AutoExterior => "Dimensions, Automatic",
            _ => "Dimensions, Manual",
        };
        if cx.layers().is_visible(layer) {
            draw_dimension(
                painter,
                cam,
                d,
                &fmt,
                Stroke::new(1.0_f32, pal.dimension_text),
                pal,
            );
        }
    }
    for c in &floor.cad {
        if cx.layers().is_visible(&c.layer) {
            draw_cad(painter, cam, &c.item, Stroke::new(1.0_f32, pal.text), pal);
        }
    }
    crate::editor::rooms_edit::draw_space_boxes(cx, painter, cam);
    crate::tools::camera::draw_camera_symbols(cx, painter, cam, None);
    if let Some(h) = cx.hover.filter(|h| !cx.selection.contains(*h)) {
        highlight(cx, painter, cam, h, Stroke::new(2.0_f32, pal.hover));
    }
    for o in &cx.selection.items {
        highlight(cx, painter, cam, *o, Stroke::new(3.0_f32, pal.selection));
    }
}

/// The crosshair lines over the whole canvas (View > Crosshairs).
pub fn draw_crosshairs(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    if !cx.view_flags.contains(&ViewFlag::Crosshairs) {
        return;
    }
    let Some(p) = cx.cursor_world else { return };
    let at = cam.world_to_screen(p);
    let rect = cam.rect;
    let stroke = Stroke::new(1.0_f32, cx.palette.text.gamma_multiply(0.45));
    painter.line_segment(
        [Pos2::new(rect.left(), at.y), Pos2::new(rect.right(), at.y)],
        stroke,
    );
    painter.line_segment(
        [Pos2::new(at.x, rect.top()), Pos2::new(at.x, rect.bottom())],
        stroke,
    );
}

fn opening_layer(o: &Opening) -> &'static str {
    match o.kind {
        OpeningKind::Door => "Doors",
        OpeningKind::Window => "Windows",
    }
}

fn quad(cam: &Camera, pts: &[Point]) -> Vec<Pos2> {
    pts.iter().map(|p| cam.world_to_screen(*p)).collect()
}

fn draw_grid(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let pal = &cx.palette;
    let mut spacing = cx.defaults.grid.spacing.max(0.25);
    while spacing * cam.px_per_in < 8.0 {
        spacing *= 5.0;
    }
    let rect = cam.rect;
    if cx.view_flags.contains(&ViewFlag::ReferenceGrid) {
        let tl = cam.screen_to_world(rect.left_top());
        let br = cam.screen_to_world(rect.right_bottom());
        let minor = Stroke::new(1.0_f32, pal.grid_minor);
        let major = Stroke::new(1.0_f32, pal.grid_major);
        let (x0, x1) = (
            (tl.x / spacing).floor() as i64,
            (br.x / spacing).ceil() as i64,
        );
        for k in x0..=x1 {
            let x = cam.world_to_screen(Point::new(k as f64 * spacing, 0.0)).x;
            let stroke = if k % 5 == 0 { major } else { minor };
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                stroke,
            );
        }
        let (y0, y1) = (
            (br.y / spacing).floor() as i64,
            (tl.y / spacing).ceil() as i64,
        );
        for k in y0..=y1 {
            let y = cam.world_to_screen(Point::new(0.0, k as f64 * spacing)).y;
            let stroke = if k % 5 == 0 { major } else { minor };
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                stroke,
            );
        }
    }
    let o = cam.world_to_screen(Point::ZERO);
    let red = Stroke::new(1.5_f32, pal.origin_marker);
    painter.line_segment([o - Vec2::new(6.0, 0.0), o + Vec2::new(6.0, 0.0)], red);
    painter.line_segment([o - Vec2::new(0.0, 6.0), o + Vec2::new(0.0, 6.0)], red);
}

fn draw_rooms(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let pal = &cx.palette;
    let outline = Stroke::new(1.5_f32, pal.room_outline);
    let show_outline = cx.layers().is_visible("Rooms");
    let show_label = cx.layers().is_visible("Room Labels");
    for room in &cx.rooms {
        if show_outline {
            painter.add(Shape::closed_line(quad(cam, &room.polygon), outline));
        }
        if show_label {
            painter.text(
                cam.world_to_screen(room.centroid),
                Align2::CENTER_CENTER,
                crate::editor::rooms_edit::room_label_text(cx, room),
                FontId::proportional(13.0),
                pal.room_label,
            );
        }
    }
    crate::editor::rooms_edit::draw_room_selection(cx, painter, cam);
}

/// The wall's drawn polygon: the mitered outline when the cache has one.
fn wall_polygon(cx: &EditorContext, wall: &Wall) -> Vec<Point> {
    cx.outlines
        .iter()
        .find(|o| o.wall_id == wall.id)
        .map_or_else(|| wall.footprint().to_vec(), |o| o.polygon.clone())
}

fn draw_walls(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let pal = &cx.palette;
    // Below this zoom the layer lines would just turn into a smear.
    let show_layers = cam.px_per_in >= 1.0;
    for wall in &cx.floor().walls {
        // Invisible walls (stairwell dividers) are room boundaries only.
        if wall.flags.invisible || !cx.layers().is_visible(&wall.layer) {
            continue;
        }
        let fill = match wall.kind {
            WallKind::Exterior => pal.wall_fill_exterior,
            WallKind::Interior => pal.wall_fill_interior,
        };
        // The mitered outline: fill and heavy edge of the whole wall.
        painter.add(Shape::convex_polygon(
            quad(cam, &wall_polygon(cx, wall)),
            fill,
            Stroke::new(1.0_f32, pal.wall_stroke),
        ));
        if !show_layers {
            continue;
        }
        // Chief's plan view of a wall type: every layer boundary as a thin
        // line, the main layer filled darker with heavier faces.
        let thin = Stroke::new(0.75_f32, pal.wall_stroke);
        let heavy = Stroke::new(1.5_f32, pal.wall_stroke);
        let main_fill = crate::theme::scale(fill, 0.78);
        for l in cx.layer_outlines.iter().filter(|l| l.wall_id == wall.id) {
            let pts = quad(cam, &l.polygon);
            if l.is_main {
                painter.add(Shape::convex_polygon(pts.clone(), main_fill, Stroke::NONE));
            }
            painter.add(Shape::closed_line(pts.clone(), thin));
            if l.is_main && pts.len() == 4 {
                // Start-left to end-left and end-right to start-right.
                painter.line_segment([pts[0], pts[1]], heavy);
                painter.line_segment([pts[2], pts[3]], heavy);
            }
        }
    }
}

/// Outline of `wall` (following the mitered corners) in `stroke`.
pub fn draw_wall_outline(
    painter: &egui::Painter,
    cam: &Camera,
    cx: &EditorContext,
    wall: &Wall,
    stroke: Stroke,
) {
    painter.add(Shape::closed_line(
        quad(cam, &wall_polygon(cx, wall)),
        stroke,
    ));
}

/// An opening in its wall: the wall is cut, jambs are drawn, then the door
/// leaf and swing arc or the window lines. `ghost` draws a placement preview.
pub fn draw_opening(
    painter: &egui::Painter,
    cam: &Camera,
    wall: &Wall,
    o: &Opening,
    pal: &Palette,
    ghost: bool,
) {
    let sc = |p: Point| cam.world_to_screen(p);
    let (line_col, arc_col) = if ghost {
        (pal.ghost_stroke, pal.ghost_stroke)
    } else {
        (pal.opening_line, pal.door_arc)
    };
    let line = Stroke::new(1.0_f32, line_col);
    let d = wall.direction();
    let n = wall.normal();
    let half = wall.thickness * 0.5;
    let over = half + 1.0 + 1.0 / cam.px_per_in;
    let pa = wall.point_at(o.start_offset());
    let pb = wall.point_at(o.end_offset());

    // A canvas-colored quad hides the wall fill and its stroke across the opening.
    let gap = [
        pa.add(n.scale(over)),
        pb.add(n.scale(over)),
        pb.sub(n.scale(over)),
        pa.sub(n.scale(over)),
    ];
    painter.add(Shape::convex_polygon(
        quad(cam, &gap),
        pal.background,
        Stroke::NONE,
    ));
    for j in [pa, pb] {
        painter.line_segment([sc(j.add(n.scale(half))), sc(j.sub(n.scale(half)))], line);
    }

    match o.kind {
        OpeningKind::Door => {
            // The hinge jamb comes from `hinge_at_end`, the swing side from
            // `swing_flipped`.
            let (hinge, toward_other) = if o.hinge_at_end {
                (pb, d.scale(-1.0))
            } else {
                (pa, d)
            };
            let side = if o.swing_flipped { n.scale(-1.0) } else { n };

            painter.line_segment([sc(hinge), sc(hinge.add(side.scale(o.width)))], line);
            let arc: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = i as f64 / 16.0 * std::f64::consts::FRAC_PI_2;
                    let v = toward_other.scale(a.cos()).add(side.scale(a.sin()));
                    sc(hinge.add(v.scale(o.width)))
                })
                .collect();
            painter.add(Shape::line(arc, Stroke::new(1.0_f32, arc_col)));
        }
        OpeningKind::Window => {
            for off in [half, 0.0, -half] {
                let s = n.scale(off);
                painter.line_segment(
                    [sc(pa.add(s)), sc(pb.add(s))],
                    Stroke::new(0.8_f32, line_col),
                );
            }
        }
    }
}

/// A dimension: extension lines, the dimension line with ticks, and the value.
pub fn draw_dimension(
    painter: &egui::Painter,
    cam: &Camera,
    d: &Dimension,
    fmt: &plan_core::DimFormat,
    stroke: Stroke,
    pal: &Palette,
) {
    for (a, b) in d.extension_lines() {
        painter.line_segment(
            [cam.world_to_screen(a), cam.world_to_screen(b)],
            Stroke::new(0.7_f32, stroke.color.gamma_multiply(0.8)),
        );
    }
    let (p, q) = d.line_points();
    let (sp, sq) = (cam.world_to_screen(p), cam.world_to_screen(q));
    painter.line_segment([sp, sq], stroke);
    let dir = (sq - sp).normalized();
    let tick = Vec2::new(-dir.y, dir.x) * 4.0;
    for s in [sp, sq] {
        painter.line_segment([s - tick, s + tick], stroke);
    }
    let galley =
        painter.layout_no_wrap(d.label(fmt), FontId::proportional(12.0), pal.dimension_text);
    let mid = Pos2::new((sp.x + sq.x) * 0.5, (sp.y + sq.y) * 0.5);
    let r = Rect::from_center_size(mid, galley.size() + Vec2::new(6.0, 2.0));
    painter.rect_filled(r, 2.0, pal.background.gamma_multiply(0.85));
    painter.galley(r.center() - galley.size() * 0.5, galley, pal.dimension_text);
}

/// A CAD primitive in `stroke`.
pub fn draw_cad(
    painter: &egui::Painter,
    cam: &Camera,
    item: &CadItem,
    stroke: Stroke,
    pal: &Palette,
) {
    let sc = |p: Point| cam.world_to_screen(p);
    match item {
        CadItem::Line { a, b } => {
            painter.line_segment([sc(*a), sc(*b)], stroke);
        }
        CadItem::Circle { center, radius } => {
            painter.circle_stroke(sc(*center), (*radius * cam.px_per_in) as f32, stroke);
        }
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
            let pts: Vec<Pos2> = (0..=48)
                .map(|i| {
                    let a = start_angle + sweep * i as f64 / 48.0;
                    sc(Point::new(
                        center.x + radius * a.cos(),
                        center.y + radius * a.sin(),
                    ))
                })
                .collect();
            painter.add(Shape::line(pts, stroke));
        }
        CadItem::Polyline { points, closed } => {
            let pts = quad(cam, points);
            painter.add(if *closed {
                Shape::closed_line(pts, stroke)
            } else {
                Shape::line(pts, stroke)
            });
        }
        CadItem::Text {
            pos, text, height, ..
        } => {
            painter.text(
                sc(*pos),
                Align2::LEFT_BOTTOM,
                text,
                FontId::proportional(((*height * cam.px_per_in) as f32).clamp(6.0, 200.0)),
                pal.text,
            );
        }
    }
}

/// Highlights one object (selection or hover).
fn highlight(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    o: ObjectRef,
    stroke: Stroke,
) {
    let floor = cx.floor();
    match o {
        ObjectRef::Wall(id) => {
            if let Some(w) = floor.wall(id) {
                draw_wall_outline(painter, cam, cx, w, stroke);
            }
        }
        ObjectRef::Opening(id) => {
            let Some(op) = floor.openings.iter().find(|x| x.id == id) else {
                return;
            };
            let Some(w) = floor.wall(op.wall_id) else {
                return;
            };
            let n = w.normal().scale(w.thickness * 0.5 + 1.0);
            let (pa, pb) = (w.point_at(op.start_offset()), w.point_at(op.end_offset()));
            painter.add(Shape::closed_line(
                quad(cam, &[pa.add(n), pb.add(n), pb.sub(n), pa.sub(n)]),
                stroke,
            ));
            // The host wall, softly (S-110).
            draw_wall_outline(painter, cam, cx, w, Stroke::new(1.0_f32, stroke.color));
        }
        ObjectRef::Dimension(id) => {
            if let Some(d) = floor.dimensions.iter().find(|d| d.id == id) {
                let (p, q) = d.line_points();
                painter.line_segment([cam.world_to_screen(p), cam.world_to_screen(q)], stroke);
            }
        }
        ObjectRef::Cad(id) | ObjectRef::Text(id) => {
            if let Some(c) = floor.cad.iter().find(|c| c.id == id) {
                draw_cad(painter, cam, &c.item, stroke, &cx.palette);
            }
        }
        ObjectRef::Device(id) => {
            let layer = crate::editor::site_view::electrical_layer(cx.floor, floor);
            if let Some(d) = layer.device(id) {
                let r = (8.0 * cam.px_per_in as f32).max(8.0);
                painter.circle_stroke(cam.world_to_screen(d.position), r, stroke);
            }
        }
        ObjectRef::RoofPlane(id) => {
            if let Some(r) = crate::editor::roof_view::load(floor).plane(id) {
                painter.add(Shape::closed_line(quad(cam, &r.plan_polygon()), stroke));
            }
        }
        ObjectRef::Camera(id) => {
            if let Some(c) = cx.project.camera(id) {
                painter.circle_stroke(cam.world_to_screen(c.position), 14.0, stroke);
            }
        }
        // Stairs, cabinets and symbols draw their own selection.
        _ => {}
    }
}

/// The snap marker: square = endpoint, triangle = midpoint, cross =
/// intersection, circle otherwise (W-13).
pub fn draw_snap_marker(painter: &egui::Painter, cam: &Camera, s: &SnapResult, color: Color32) {
    let c = cam.world_to_screen(s.point);
    let stroke = Stroke::new(1.5_f32, color);
    match s.kind {
        SnapKind::Endpoint => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(10.0)),
                0.0,
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        SnapKind::Midpoint => {
            painter.add(Shape::closed_line(
                vec![
                    c + Vec2::new(0.0, -6.0),
                    c + Vec2::new(6.0, 5.0),
                    c + Vec2::new(-6.0, 5.0),
                ],
                stroke,
            ));
        }
        SnapKind::Intersection => {
            painter.line_segment([c + Vec2::new(-5.0, -5.0), c + Vec2::new(5.0, 5.0)], stroke);
            painter.line_segment([c + Vec2::new(-5.0, 5.0), c + Vec2::new(5.0, -5.0)], stroke);
        }
        _ => {
            painter.circle_stroke(c, 5.0, stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    /// Draws a plan with every object kind, a selection and a hover through a
    /// headless egui context; a panic here means a drawing bug.
    #[test]
    fn draws_every_object_kind_without_panicking() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 144.0),
            Point::new(0.0, 144.0),
        ];
        let mut ids = Vec::new();
        for i in 0..4 {
            ids.push(cx.project.add_wall(
                0,
                c[i],
                c[(i + 1) % 4],
                7.625,
                109.0,
                WallKind::Exterior,
            ));
        }
        for id in &ids {
            if let Some(w) = cx.project.floors[0].wall_mut(*id) {
                w.wall_type = Some("Stucco-6".into());
            }
        }
        let door = cx
            .project
            .add_opening(0, ids[0], 100.0, OpeningKind::Door)
            .unwrap();
        cx.project
            .add_opening(0, ids[2], 100.0, OpeningKind::Window)
            .unwrap();
        cx.project.add_dimension(
            0,
            Dimension::new(0, DimensionKind::Manual, c[0], c[1], 24.0),
        );
        for item in [
            CadItem::Line { a: c[0], b: c[2] },
            CadItem::Circle {
                center: c[0],
                radius: 12.0,
            },
            CadItem::Arc {
                center: c[1],
                radius: 20.0,
                start_angle: 0.0,
                end_angle: 2.0,
            },
            CadItem::Polyline {
                points: c.to_vec(),
                closed: true,
            },
            CadItem::Text {
                pos: c[3],
                text: "Note".into(),
                height: 3.0,
                angle: 0.0,
            },
        ] {
            cx.project.add_cad(0, "CAD, Default", item);
        }
        cx.selection.set(ObjectRef::Opening(door));
        cx.hover = Some(ObjectRef::Wall(ids[1]));
        cx.refresh();
        assert_eq!(cx.rooms.len(), 1);
        // A Stucco-6 wall is drawn as several layer polygons.
        assert!(
            cx.layer_outlines
                .iter()
                .filter(|l| l.wall_id == ids[0])
                .count()
                > 1
        );

        let egui_ctx = egui::Context::default();
        let _ = egui_ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, painter) =
                    ui.allocate_painter(Vec2::new(800.0, 600.0), egui::Sense::hover());
                let mut cam = Camera::default_view();
                cam.rect = painter.clip_rect();
                for scale in [0.5, 2.0] {
                    cam.px_per_in = scale;
                    draw_plan(&cx, &painter, &cam);
                    crate::editor::handles::draw(
                        &crate::editor::handles::handles_for(&cx, scale),
                        &painter,
                        &cam,
                        &cx.palette,
                    );
                }
            });
        });
    }
}
