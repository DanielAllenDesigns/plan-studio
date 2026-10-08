//! Plan drawing: grid, rooms, walls (mitered outlines from
//! `plan_core::joins`), openings, dimensions, CAD items, selection and hover
//! highlights. Layer visibility comes from `project.layers`. Tools draw their
//! own overlays on top (`Tool::draw_overlay`).

use super::restyle;
use super::selection::ObjectRef;
use super::sheet;
use super::snap::{SnapKind, SnapResult};
use super::{Camera, EditorContext};
use crate::theme::Palette;
use crate::toolbar::ViewFlag;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::{Dimension, DimensionKind, Opening, OpeningKind, Wall, WallClass, WallKind};

/// Everything under the tool overlay.
pub fn draw_plan(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let pal = &cx.palette;
    painter.rect_filled(cam.rect, 0.0, pal.background);
    // Everything from here to the highlights goes through View > Color.
    let content = restyle::mark(painter);
    draw_grid(cx, painter, cam);
    draw_reference_floor(cx, painter, cam);
    crate::editor::site_view::draw_site(cx, painter, cam);
    draw_rooms(cx, painter, cam);
    // Slabs, pads and piers sit under the walls.
    crate::editor::foundation_view::draw_foundation(cx, painter, cam);
    // Floor material regions, decks and 3D solids sit under the walls too.
    crate::editor::details_view::draw_under(cx, painter, cam);
    crate::editor::stairs_view::draw_stairs(cx, painter, cam);
    crate::editor::placed::draw_placed(cx, painter, cam);
    crate::editor::roof_view::draw_roofs(cx, painter, cam);
    draw_walls(cx, painter, cam);
    // Wall hatching and wall regions, corner trim and moldings over the walls.
    crate::editor::details_view::draw_over(cx, painter, cam);
    let floor = cx.floor();
    for wall in &floor.walls {
        for o in floor.openings_on(wall.id) {
            if cx.layers().is_visible(opening_layer(o)) {
                weighted(cx, painter, opening_layer(o), || {
                    draw_opening(painter, cam, wall, o, pal, false)
                });
            }
        }
    }
    // Devices sit on the wall faces: over the wall fill and the openings.
    crate::editor::site_view::draw_devices(cx, painter, cam);
    crate::editor::framing_view::draw(cx, painter, cam);
    let fmt = cx.dim_format();
    for d in &floor.dimensions {
        let layer = match d.kind {
            DimensionKind::AutoExterior => "Dimensions, Automatic",
            _ => "Dimensions, Manual",
        };
        if cx.layers().is_visible(layer) {
            weighted(cx, painter, layer, || {
                draw_dimension(
                    painter,
                    cam,
                    d,
                    &fmt,
                    Stroke::new(1.0_f32, pal.dimension_text),
                    pal,
                )
            });
        }
    }
    let attrs = floor.cad_attr_map();
    for c in &floor.cad {
        if !cx.layers().is_visible(&c.layer) {
            continue;
        }
        match attrs.get(&c.id) {
            Some(a) => crate::tools::cad::draw_cad_styled(cx, painter, cam, c, Some(a)),
            None => weighted(cx, painter, &c.layer, || {
                draw_cad(painter, cam, &c.item, Stroke::new(1.0_f32, pal.text), pal)
            }),
        }
    }
    crate::editor::rooms_edit::draw_space_boxes(cx, painter, cam);
    // Placed schedule tables and their callout labels.
    crate::editor::schedule_view::draw_schedules(cx, painter, cam);
    crate::tools::camera::draw_camera_symbols(cx, painter, cam, None);
    draw_sheet(cx, painter, cam);
    if !cx.view_flags.contains(&ViewFlag::Color) {
        restyle::restyle_from(painter, content, restyle::monochrome);
    }
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
            weighted(cx, painter, "Rooms", || {
                painter.add(Shape::closed_line(quad(cam, &room.polygon), outline));
            });
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
    // The join cache only knows straight walls.
    if wall.is_curved() {
        return wall.plan_polygon();
    }
    cx.outlines
        .iter()
        .find(|o| o.wall_id == wall.id)
        .map_or_else(|| wall.footprint().to_vec(), |o| o.polygon.clone())
}

fn draw_walls(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    for wall in &cx.floor().walls {
        if !cx.layers().is_visible(&wall.layer) {
            continue;
        }
        // A drawn room divider is invisible in 3D but shows as a dashed line.
        if wall.class == WallClass::RoomDivider {
            weighted(cx, painter, &wall.layer, || {
                draw_room_divider(cx, painter, cam, wall)
            });
            continue;
        }
        // Invisible walls (stairwell dividers) are room boundaries only.
        if wall.flags.invisible {
            continue;
        }
        weighted(cx, painter, &wall.layer, || {
            draw_one_wall(cx, painter, cam, wall)
        });
    }
}

/// The wall's left and right face lines (a straight wall's footprint edges,
/// or the two offset curves of an arc), both running start to end.
fn face_lines(wall: &Wall) -> (Vec<Point>, Vec<Point>) {
    if wall.is_curved() {
        let poly = wall.plan_polygon();
        let h = poly.len() / 2;
        (
            poly[..h].to_vec(),
            poly[h..].iter().rev().copied().collect(),
        )
    } else {
        let f = wall.footprint();
        (vec![f[0], f[1]], vec![f[3], f[2]])
    }
}

/// Fills the wall (an arc as a strip of convex facets) and outlines it.
fn fill_wall(
    painter: &egui::Painter,
    cam: &Camera,
    wall: &Wall,
    poly: &[Point],
    fill: Color32,
    stroke: Stroke,
) {
    if wall.is_curved() {
        let (left, right) = face_lines(wall);
        for i in 0..left.len().saturating_sub(1) {
            let quad = [left[i], left[i + 1], right[i + 1], right[i]];
            painter.add(Shape::convex_polygon(
                quad.iter().map(|p| cam.world_to_screen(*p)).collect(),
                fill,
                Stroke::NONE,
            ));
        }
        painter.add(Shape::closed_line(quad(cam, poly), stroke));
    } else {
        painter.add(Shape::convex_polygon(quad(cam, poly), fill, stroke));
    }
}

/// Diagonal hatch lines (screen space, 45 degrees) inside `poly`.
fn hatch_polygon(painter: &egui::Painter, poly: &[Pos2], spacing: f32, stroke: Stroke) {
    if poly.len() < 3 {
        return;
    }
    let sums = poly.iter().map(|p| p.x + p.y);
    let lo = sums.clone().fold(f32::MAX, f32::min);
    let hi = sums.fold(f32::MIN, f32::max);
    let mut c = (lo / spacing).ceil() * spacing;
    while c < hi {
        let mut hits: Vec<Pos2> = Vec::new();
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let (fa, fb) = (a.x + a.y - c, b.x + b.y - c);
            if (fa < 0.0) != (fb < 0.0) {
                hits.push(a + (b - a) * (fa / (fa - fb)));
            }
        }
        hits.sort_by(|p, q| p.x.total_cmp(&q.x));
        for i in (0..hits.len().saturating_sub(1)).step_by(2) {
            painter.line_segment([hits[i], hits[i + 1]], stroke);
        }
        c += spacing;
    }
}

/// Room divider: a thin dashed line on its layer.
fn draw_room_divider(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, wall: &Wall) {
    let pts: Vec<Pos2> = wall
        .sample_points(if wall.is_curved() { 24 } else { 1 })
        .iter()
        .map(|p| cam.world_to_screen(*p))
        .collect();
    let stroke = Stroke::new(0.75_f32, cx.palette.wall_stroke.gamma_multiply(0.7));
    painter.extend(Shape::dashed_line(&pts, stroke, 6.0, 4.0));
}

/// Glass walls and railings in plan: two thin face lines, and for railings
/// a post at each end.
fn draw_double_line(
    painter: &egui::Painter,
    cam: &Camera,
    wall: &Wall,
    color: Color32,
    posts: bool,
) {
    let (left, right) = face_lines(wall);
    let stroke = Stroke::new(0.75_f32, color);
    for line in [&left, &right] {
        painter.add(Shape::line(
            line.iter().map(|p| cam.world_to_screen(*p)).collect(),
            stroke,
        ));
    }
    if posts {
        let r = (wall.thickness.max(3.5) * 0.5 * cam.px_per_in).max(1.5) as f32;
        for p in [wall.start, wall.end] {
            painter.circle_filled(cam.world_to_screen(p), r, color);
        }
    }
}

/// Fencing in plan: a center line with a post at least every 96".
fn draw_fence(painter: &egui::Painter, cam: &Camera, wall: &Wall) {
    let color = Color32::from_rgb(90, 130, 60);
    let pts: Vec<Pos2> = wall
        .sample_points(if wall.is_curved() { 24 } else { 1 })
        .iter()
        .map(|p| cam.world_to_screen(*p))
        .collect();
    painter.add(Shape::line(pts, Stroke::new(1.25_f32, color)));
    let bays = (wall.path_length() / 96.0).ceil().max(1.0) as usize;
    let r = (1.75 * cam.px_per_in).max(1.5) as f32;
    for p in wall.sample_points(bays) {
        painter.circle_filled(cam.world_to_screen(p), r, color);
    }
}

/// The layers of a pony wall's lower type, drawn straight across the wall.
fn draw_pony_lower(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    wall: &Wall,
    lower_type: &str,
    fill: Color32,
) {
    let Some(def) = cx
        .project
        .wall_type_def(lower_type)
        .or_else(|| cx.defaults.wall_type(lower_type))
    else {
        return;
    };
    let mut lower = wall.clone();
    lower.thickness = def.thickness();
    let n = lower.normal();
    let thin = Stroke::new(0.75_f32, cx.palette.wall_stroke);
    for b in plan_core::wall_layer_bands(&lower, Some(def)) {
        let pts: Vec<Pos2> = [
            wall.start.add(n.scale(b.outer)),
            wall.end.add(n.scale(b.outer)),
            wall.end.add(n.scale(b.inner)),
            wall.start.add(n.scale(b.inner)),
        ]
        .iter()
        .map(|p| cam.world_to_screen(*p))
        .collect();
        let color = if b.is_main {
            crate::theme::scale(fill, 0.78)
        } else {
            fill
        };
        painter.add(Shape::convex_polygon(pts, color, thin));
    }
}

fn draw_one_wall(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, wall: &Wall) {
    let pal = &cx.palette;
    // Below this zoom the layer lines would just turn into a smear.
    let show_layers = cam.px_per_in >= 1.0;
    let mut fill = match wall.kind {
        WallKind::Exterior => pal.wall_fill_exterior,
        WallKind::Interior => pal.wall_fill_interior,
    };
    // Wall classes with their own plan symbol (W-52..W-58).
    match &wall.class {
        WallClass::Glass | WallClass::GlassPony { .. } => {
            return draw_double_line(painter, cam, wall, Color32::from_rgb(70, 130, 180), false);
        }
        WallClass::Railing | WallClass::DeckRailing => {
            return draw_double_line(painter, cam, wall, pal.wall_stroke, true);
        }
        WallClass::Fencing { .. } => return draw_fence(painter, cam, wall),
        WallClass::DeckEdge => fill = Color32::from_rgb(196, 160, 110),
        _ => {}
    }
    let poly = wall_polygon(cx, wall);
    // The mitered outline: fill and heavy edge of the whole wall.
    fill_wall(
        painter,
        cam,
        wall,
        &poly,
        fill,
        Stroke::new(
            if wall.class == WallClass::DeckEdge {
                1.75_f32
            } else {
                1.0_f32
            },
            pal.wall_stroke,
        ),
    );
    if !show_layers || wall.class == WallClass::DeckEdge {
        return;
    }
    // Foundation walls are hatched concrete.
    if wall.is_foundation() {
        let hatch = Stroke::new(0.75_f32, pal.wall_stroke.gamma_multiply(0.6));
        hatch_polygon(painter, &quad(cam, &poly), 6.0, hatch);
    }
    // A pony wall's plan shows the lower type unless the upper one sets it.
    if let WallClass::Pony {
        lower_type,
        upper_sets_plan_display: false,
        ..
    } = &wall.class
    {
        return draw_pony_lower(cx, painter, cam, wall, lower_type, fill);
    }
    if wall.is_curved() {
        return;
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

/// Runs `draw` and, with View > Line Weights on, scales the stroke widths it
/// produced by the line weight of `layer` (0.25 mm is the base width).
fn weighted(cx: &EditorContext, painter: &egui::Painter, layer: &str, draw: impl FnOnce()) {
    if !cx.view_flags.contains(&ViewFlag::LineWeights) {
        draw();
        return;
    }
    let start = restyle::mark(painter);
    draw();
    if let Some(l) = cx.layers().get(layer) {
        let k = restyle::weight_factor(l.line_weight);
        if (k - 1.0).abs() > 1e-3 {
            restyle::restyle_from(painter, start, |s| restyle::scale_widths(s, k));
        }
    }
}

/// The walls of the floor below as plan polygons (View > Reference Display).
/// Nothing on the lowest floor.
pub fn reference_polygons(cx: &EditorContext) -> Vec<Vec<Point>> {
    if !cx.view_flags.contains(&ViewFlag::ReferenceDisplay) || cx.floor == 0 {
        return Vec::new();
    }
    cx.project.floors[cx.floor - 1]
        .walls
        .iter()
        .filter(|w| !w.flags.invisible && cx.layers().is_visible(&w.layer))
        .map(|w| w.footprint().to_vec())
        .collect()
}

/// Reference Display: the floor below, walls only, in gray.
fn draw_reference_floor(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let fill = Color32::from_rgba_unmultiplied(128, 128, 128, 70);
    let edge = Stroke::new(
        0.75_f32,
        Color32::from_rgba_unmultiplied(128, 128, 128, 170),
    );
    for poly in reference_polygons(cx) {
        painter.add(Shape::convex_polygon(quad(cam, &poly), fill, edge));
    }
}

/// The parts of `clip` that lie outside `sheet` (up to four rectangles).
pub fn outside_rects(clip: Rect, sheet: Rect) -> Vec<Rect> {
    let s = sheet.intersect(clip);
    if s.width() <= 0.0 || s.height() <= 0.0 {
        return vec![clip];
    }
    [
        Rect::from_min_max(clip.min, Pos2::new(clip.max.x, s.min.y)),
        Rect::from_min_max(Pos2::new(clip.min.x, s.max.y), clip.max),
        Rect::from_min_max(Pos2::new(clip.min.x, s.min.y), Pos2::new(s.min.x, s.max.y)),
        Rect::from_min_max(Pos2::new(s.max.x, s.min.y), Pos2::new(clip.max.x, s.max.y)),
    ]
    .into_iter()
    .filter(|r| r.width() > 0.0 && r.height() > 0.0)
    .collect()
}

/// View > Drawing Sheet: the active layout's sheet outline, centered on the
/// plan. View > Print Preview additionally grays out everything outside it.
fn draw_sheet(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let sheet_on = cx.view_flags.contains(&ViewFlag::DrawingSheet);
    let preview = cx.view_flags.contains(&ViewFlag::PrintPreview);
    if !sheet_on && !preview {
        return;
    }
    let (lo, hi) = cx.sheet.rect_around(sheet::plan_center(cx.floor()));
    let rect = Rect::from_two_pos(cam.world_to_screen(lo), cam.world_to_screen(hi));
    if preview {
        let shade = Color32::from_black_alpha(150);
        for r in outside_rects(cam.rect, rect) {
            painter.rect_filled(r, 0.0, shade);
        }
    }
    let stroke = Stroke::new(1.5_f32, cx.palette.text.gamma_multiply(0.8));
    painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Middle);
    painter.text(
        rect.left_top() + Vec2::new(6.0, 4.0),
        Align2::LEFT_TOP,
        cx.sheet.caption(),
        FontId::proportional(12.0),
        cx.palette.text.gamma_multiply(0.8),
    );
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
            let set = crate::editor::roof_view::load(floor);
            for (rid, _, polys) in set.pick_polys() {
                if rid == id {
                    for poly in polys {
                        painter.add(Shape::closed_line(quad(cam, &poly), stroke));
                    }
                }
            }
        }
        ObjectRef::Camera(id) => {
            if let Some(c) = cx.project.camera(id) {
                painter.circle_stroke(cam.world_to_screen(c.position), 14.0, stroke);
            }
        }
        // Stairs, cabinets, symbols and foundation objects draw their own
        // selection.
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

    #[test]
    fn outside_rects_cover_the_clip_minus_the_sheet() {
        let clip = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 80.0));
        let sheet = Rect::from_min_max(Pos2::new(20.0, 10.0), Pos2::new(70.0, 60.0));
        let rects = outside_rects(clip, sheet);
        let area: f32 = rects.iter().map(|r| r.width() * r.height()).sum();
        assert!((area - (100.0 * 80.0 - 50.0 * 50.0)).abs() < 1e-3);
        assert!(rects.iter().all(|r| !r.intersects(sheet.shrink(0.5))));
        // A sheet beyond the view shades nothing; one off-screen shades all.
        assert!(outside_rects(clip, clip.expand(10.0)).is_empty());
        assert_eq!(
            outside_rects(clip, clip.translate(Vec2::new(500.0, 0.0))),
            vec![clip]
        );
    }

    #[test]
    fn reference_display_shows_the_floor_below_only() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project.build_new_floor(false);
        cx.project.add_wall(
            0,
            Point::ZERO,
            Point::new(120.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        cx.floor = 1;
        assert!(reference_polygons(&cx).is_empty(), "flag off");
        cx.view_flags.insert(ViewFlag::ReferenceDisplay);
        assert_eq!(reference_polygons(&cx).len(), 1);
        cx.floor = 0;
        assert!(
            reference_polygons(&cx).is_empty(),
            "nothing below the first floor"
        );
    }

    /// With the flags on the plan still draws, and Color off leaves only
    /// gray strokes on the walls.
    #[test]
    fn view_toggles_restyle_the_plan() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project.add_wall(
            0,
            Point::ZERO,
            Point::new(240.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        cx.refresh();
        let colored = |cx: &EditorContext| {
            let mut found = false;
            let egui_ctx = egui::Context::default();
            let _ = egui_ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (_, painter) =
                        ui.allocate_painter(Vec2::new(400.0, 300.0), egui::Sense::hover());
                    let mut cam = Camera::default_view();
                    cam.rect = painter.clip_rect();
                    draw_plan(cx, &painter, &cam);
                    painter.for_each_shape(|cs| {
                        if let Shape::LineSegment { stroke, .. } = &cs.shape {
                            let c = stroke.color;
                            if c.a() > 0 && !(c.r() == c.g() && c.g() == c.b()) {
                                found = true;
                            }
                        }
                    });
                });
            });
            found
        };
        assert!(colored(&cx), "the origin marker is red in color mode");
        cx.view_flags.remove(&ViewFlag::Color);
        assert!(!colored(&cx), "everything is gray with Color off");
        cx.view_flags.extend([
            ViewFlag::Color,
            ViewFlag::LineWeights,
            ViewFlag::DrawingSheet,
            ViewFlag::PrintPreview,
            ViewFlag::ReferenceDisplay,
        ]);
        let _ = colored(&cx);
    }

    /// Plan shapes of one 240" wall of `class`: (filled paths, line segments).
    fn wall_shapes(class: WallClass, curved: bool) -> (usize, usize) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_wall(
            0,
            Point::ZERO,
            Point::new(240.0, 0.0),
            7.625,
            109.0,
            WallKind::Exterior,
        );
        let w = cx.project.floors[0].wall_mut(id).unwrap();
        if let Some(l) = class.default_layer() {
            w.layer = l.to_string();
        }
        w.set_class(class);
        w.wall_type = Some("Stucco-6".into());
        if curved {
            w.curve = Some(plan_core::WallCurve { bulge: 40.0 });
        }
        cx.refresh();
        let (mut filled, mut lines) = (0, 0);
        let egui_ctx = egui::Context::default();
        let _ = egui_ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, painter) =
                    ui.allocate_painter(Vec2::new(800.0, 600.0), egui::Sense::hover());
                let mut cam = Camera::default_view();
                cam.rect = painter.clip_rect();
                cam.px_per_in = 2.0;
                draw_walls(&cx, &painter, &cam);
                painter.for_each_shape(|cs| match &cs.shape {
                    Shape::Path(p) if p.fill != Color32::TRANSPARENT => filled += 1,
                    Shape::LineSegment { .. } => lines += 1,
                    _ => {}
                });
            });
        });
        (filled, lines)
    }

    #[test]
    fn wall_classes_have_their_own_plan_symbols() {
        let (std_fill, std_lines) = wall_shapes(WallClass::Standard, false);
        assert!(std_fill >= 1);
        // Foundation walls add a concrete hatch.
        let (f_fill, f_lines) = wall_shapes(WallClass::Foundation, false);
        assert!(
            f_fill >= 1 && f_lines > std_lines + 5,
            "{f_lines} {std_lines}"
        );
        // Glass, railings, fences and room dividers are lines, not filled walls.
        for class in [
            WallClass::Glass,
            WallClass::GlassPony {
                lower_type: "Brick-6".into(),
                split_height: 36.0,
            },
            WallClass::Railing,
            WallClass::DeckRailing,
            WallClass::Fencing {
                style: plan_core::FenceStyle::Picket,
            },
            WallClass::RoomDivider,
        ] {
            let (fill, _) = wall_shapes(class.clone(), false);
            assert_eq!(fill, 0, "{class:?}");
        }
        // A pony wall shows its lower type's layers unless the upper sets the plan.
        let pony = |upper: bool| WallClass::Pony {
            upper_type: "Stucco-6".into(),
            lower_type: "Brick-6".into(),
            split_height: 36.0,
            upper_sets_plan_display: upper,
        };
        let (lower_fill, _) = wall_shapes(pony(false), false);
        let (upper_fill, _) = wall_shapes(pony(true), false);
        assert_ne!(lower_fill, upper_fill);
        // Half-walls and deck edges draw as walls; every class draws curved.
        assert!(wall_shapes(WallClass::HalfWall { height: 36.0 }, false).0 >= 1);
        assert!(wall_shapes(WallClass::DeckEdge, false).0 >= 1);
        for class in [WallClass::Standard, WallClass::Foundation, WallClass::Glass] {
            let _ = wall_shapes(class, true);
        }
        assert!(wall_shapes(WallClass::Standard, true).0 > 4, "arc facets");
    }

    #[test]
    fn face_lines_follow_the_arc() {
        let mut w = Wall::new(
            Point::ZERO,
            Point::new(100.0, 0.0),
            10.0,
            96.0,
            WallKind::Interior,
        );
        let (l, r) = face_lines(&w);
        assert_eq!((l.len(), r.len()), (2, 2));
        assert_eq!((l[0], r[0]), (Point::new(0.0, 5.0), Point::new(0.0, -5.0)));
        w.curve = Some(plan_core::WallCurve { bulge: 20.0 });
        let (l, r) = face_lines(&w);
        assert_eq!(l.len(), r.len());
        assert!(l.len() > 3);
        // The faces are 5" either side of the arc, square to its tangent.
        assert!((l[0].dist(Point::ZERO) - 5.0).abs() < 1e-6);
        assert!((r[0].dist(Point::ZERO) - 5.0).abs() < 1e-6);
        assert!((r.last().unwrap().dist(Point::new(100.0, 0.0)) - 5.0).abs() < 1e-6);
    }
}
