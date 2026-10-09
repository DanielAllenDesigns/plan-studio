//! Plan drawing: grid, rooms, walls (mitered outlines from
//! `plan_core::joins`), openings, dimensions, CAD items, selection and hover
//! highlights. Layer visibility comes from `project.layers`. Tools draw their
//! own overlays on top (`Tool::draw_overlay`).

use super::opening_view::{draw_floor_opening, draw_opening_labels};
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
use plan_core::{
    exterior_sign, Dimension, DimensionKind, Opening, Wall, WallClass, WallKind,
};
use std::collections::HashMap;

/// Runs one stage of [`draw_plan`]. Test builds add the stage's time to
/// [`SECTION_MS`] so the benchmark can say where a frame goes; elsewhere it is
/// just the block.
macro_rules! section {
    ($name:literal, $($body:tt)*) => {{
        #[cfg(test)]
        let started = std::time::Instant::now();
        let out = { $($body)* };
        #[cfg(test)]
        SECTION_MS.with(|m| {
            *m.borrow_mut().entry($name).or_insert(0.0) += started.elapsed().as_secs_f64() * 1000.0
        });
        out
    }};
}

#[cfg(test)]
thread_local! {
    /// Milliseconds spent per `draw_plan` stage since the last reset.
    pub(crate) static SECTION_MS: std::cell::RefCell<std::collections::BTreeMap<&'static str, f64>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

thread_local! {
    /// The font CAD text and dimension numbers are drawn in now: the text
    /// style of the object being drawn (see [`set_text_face`]).
    static TEXT_FACE: std::cell::RefCell<Option<plan_docs::pdf::FontSpec>> =
        const { std::cell::RefCell::new(None) };
}

/// Sets the font [`draw_cad`] and the dimension numbers draw in; `None` is
/// the bundled proportional font.
pub fn set_text_face(face: Option<plan_docs::pdf::FontSpec>) {
    TEXT_FACE.with(|f| *f.borrow_mut() = face);
}

/// The font of `style` as a text face (`None` when it names no font).
fn face_of(style: Option<&plan_core::TextStyle>) -> Option<plan_docs::pdf::FontSpec> {
    plan_docs::pdf::FontSpec::of_style(style?)
}

/// The font for text drawn at `size` px: the current text face's installed
/// font, else the bundled one.
fn text_font(painter: &egui::Painter, size: f32) -> FontId {
    match TEXT_FACE.with(|f| f.borrow().clone()) {
        Some(spec) => crate::fonts::font_id(painter.ctx(), &spec, size),
        None => FontId::proportional(size),
    }
}

/// The text style a dimension's number is set in, as [`DimLook::of`] finds
/// it: the dimension's own, else the active set's, else "Dimension Text
/// Style"; the plan's styles before the defaults'.
fn dimension_style<'a>(cx: &'a EditorContext, d: &Dimension) -> Option<&'a plan_core::TextStyle> {
    let set = &cx.defaults.dimensions;
    let name = d
        .text_style
        .as_deref()
        .filter(|n| !n.is_empty())
        .or(Some(set.text_style.as_str()).filter(|n| !n.is_empty()))
        .unwrap_or("Dimension Text Style");
    cx.project
        .text_styles
        .resolve(name)
        .or_else(|| cx.defaults.text_styles.resolve(name))
}

/// Everything under the tool overlay.
pub fn draw_plan(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let pal = &cx.palette;
    painter.rect_filled(cam.rect, 0.0, pal.background);
    // Everything from here to the highlights goes through View > Color.
    let content = restyle::mark(painter);
    section!("grid", draw_grid(cx, painter, cam));
    section!(
        "underlays",
        crate::tools::underlay::draw_underlays(cx, painter, cam)
    );
    section!(
        "reference floor",
        crate::editor::ref_overlay::draw_reference(cx, painter, cam, false)
    );
    section!(
        "site",
        crate::editor::site_view::draw_site(cx, painter, cam)
    );
    section!("rooms", draw_rooms(cx, painter, cam));
    // CAD objects in a drawing group below the walls' draw under them.
    section!("cad behind walls", draw_cad_pass(cx, painter, cam, true));
    // Construction lines in a drawing group below the walls' (group 21).
    section!(
        "construction lines behind walls",
        crate::editor::ref_overlay::draw_construction(cx, painter, cam, true)
    );
    // Slabs, pads and piers sit under the walls.
    section!(
        "foundation",
        crate::editor::foundation_view::draw_foundation(cx, painter, cam)
    );
    // Floor material regions, decks and 3D solids sit under the walls too.
    // Fill Styles assigned to slabs and rooms (Fill Style panel).
    section!("assigned fills", draw_assigned_fills(cx, painter, cam));
    section!(
        "details under",
        crate::editor::details_view::draw_under(cx, painter, cam)
    );
    section!(
        "stairs",
        crate::editor::stairs_view::draw_stairs(cx, painter, cam)
    );
    section!(
        "placed",
        crate::editor::placed::draw_placed(cx, painter, cam)
    );
    // Compound 3D solids, and the boxes and labels of architectural blocks.
    section!(
        "blocks and solids",
        crate::editor::solids_view::draw(cx, painter, cam)
    );
    section!(
        "roofs",
        crate::editor::roof_view::draw_roofs(cx, painter, cam)
    );
    section!("walls", draw_walls(cx, painter, cam));
    // Poché: the dark fill over the cut walls (View > Poché).
    section!("poche", draw_poche(cx, painter, cam));
    // Wall hatching and wall regions, corner trim and moldings over the walls.
    section!(
        "details over",
        crate::editor::details_view::draw_over(cx, painter, cam)
    );
    let floor = cx.floor();
    section!(
        "openings",
        for wall in &floor.walls {
            let mut outside = None;
            for o in floor.openings_on(wall.id) {
                if cx.layers().is_visible(opening_layer(o)) {
                    let ext = *outside.get_or_insert_with(|| exterior_sign(wall, &cx.rooms));
                    weighted(cx, painter, opening_layer(o), || {
                        draw_floor_opening(painter, cam, floor, wall, o, pal, ext)
                    });
                }
            }
        }
    );
    section!("opening labels", draw_opening_labels(cx, painter, cam));
    // Devices sit on the wall faces: over the wall fill and the openings.
    section!(
        "devices",
        crate::editor::site_view::draw_devices(cx, painter, cam)
    );
    section!(
        "framing",
        crate::editor::framing_view::draw(cx, painter, cam)
    );
    let fmt = cx.dim_format();
    section!(
        "dimensions",
        for d in &floor.dimensions {
            let layer = match d.kind {
                DimensionKind::AutoExterior => "Dimensions, Automatic",
                _ => "Dimensions, Manual",
            };
            if cx.layers().is_visible(layer) {
                let look = DimLook::of(cx, d);
                set_text_face(face_of(dimension_style(cx, d)));
                weighted(cx, painter, layer, || {
                    draw_dimension_look(
                        painter,
                        cam,
                        d,
                        &fmt,
                        Stroke::new(1.0_f32, pal.dimension_text),
                        pal,
                        &look,
                    )
                });
                set_text_face(None);
            }
        }
    );
    // CAD objects whose drawing group is above the walls' draw over everything.
    section!("cad", draw_cad_pass(cx, painter, cam, false));
    section!(
        "construction lines",
        crate::editor::ref_overlay::draw_construction(cx, painter, cam, false)
    );
    // Reference rows above the Current line draw over the plan; a wall drawn
    // exactly over its reference counterpart gets light blue edges (S-194).
    section!(
        "reference front",
        crate::editor::ref_overlay::draw_reference(cx, painter, cam, true)
    );
    section!(
        "reference alignment",
        crate::editor::ref_overlay::draw_alignment(cx, painter, cam)
    );
    section!(
        "space boxes",
        crate::editor::rooms_edit::draw_space_boxes(cx, painter, cam)
    );
    // Placed schedule tables and their callout labels.
    section!(
        "schedules",
        crate::editor::schedule_view::draw_schedules(cx, painter, cam)
    );
    section!(
        "camera symbols",
        crate::tools::camera::draw_camera_symbols(cx, painter, cam, None)
    );
    draw_sheet(cx, painter, cam);
    if !cx.view_flags.contains(&ViewFlag::Color) {
        restyle::restyle_from(painter, content, restyle::monochrome);
    }
    // Print Preview shows the colour mode the Print dialog chose.
    if cx.view_flags.contains(&ViewFlag::PrintPreview) {
        match sheet::preview_color() {
            plan_layout::PrintColor::Color => {}
            plan_layout::PrintColor::Grayscale => {
                restyle::restyle_from(painter, content, restyle::monochrome);
            }
            plan_layout::PrintColor::BlackWhite => {
                restyle::restyle_from(painter, content, restyle::black_and_white);
            }
        }
    }
    if let Some(h) = cx.hover.filter(|h| !cx.selection.contains(*h)) {
        highlight(cx, painter, cam, h, Stroke::new(2.0_f32, pal.hover));
    }
    for o in &cx.selection.items {
        highlight(cx, painter, cam, *o, Stroke::new(3.0_f32, pal.selection));
    }
}

/// The CAD objects (and text) in drawing-group order: with `behind` the ones
/// whose group is below the walls' (they draw under the walls), otherwise the
/// rest, over everything else (LAY-36, `plan_core::drawing_group`).
fn draw_cad_pass(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, behind: bool) {
    let pal = &cx.palette;
    let floor = cx.floor();
    let (under, over) = floor.cad_by_walls(&cx.project.drawing_group_defaults);
    let objects = if behind { under } else { over };
    if objects.is_empty() {
        return;
    }
    let attrs = floor.cad_attr_map();
    // Construction lines are drawn by `ref_overlay` (infinite, with callouts).
    let construction = floor.construction_ids();
    // Text objects are drawn in their text style's font; anything drawn
    // after them (tool previews) in the Default Text Style's.
    let default_face = face_of(
        cx.project
            .text_styles
            .resolve(plan_core::text_styles::DEFAULT_TEXT_STYLE_NAME),
    );
    for c in objects {
        if !cx.layers().is_visible(&c.layer) || construction.contains(&c.id) {
            continue;
        }
        if matches!(c.item, CadItem::Text { .. }) {
            let style = cx.project.text_styles.style_of_text(
                cx.layers(),
                &c.layer,
                attrs.get(&c.id).and_then(|a| a.text_style.as_deref()),
            );
            set_text_face(face_of(style));
        }
        // A library line style (Line Style Management) and an assigned Fill
        // Style draw through the shared stroker and tiler.
        if draw_styled_cad(cx, painter, cam, c, attrs.get(&c.id)) {
            continue;
        }
        // Text in a printed-size style is drawn at its size on paper for
        // the sheet's scale.
        let printed = printed_text_object(cx, c, attrs.get(&c.id));
        let c = printed.as_ref().unwrap_or(c);
        match attrs.get(&c.id) {
            Some(a) if matches!(c.item, CadItem::Text { .. }) && a.text_box.needs_layout() => {
                draw_text_box(cx, painter, cam, c, a)
            }
            Some(a) => crate::tools::cad::draw_cad_styled(cx, painter, cam, c, Some(a)),
            None => weighted(cx, painter, &c.layer, || {
                draw_cad(painter, cam, &c.item, Stroke::new(1.0_f32, pal.text), pal)
            }),
        }
    }
    set_text_face(default_face);
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

/// The layer an opening draws on: its Layer tab's choice, else Doors or Windows.
fn opening_layer(o: &Opening) -> &str {
    o.layer_name()
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
    if cx.view_flags.contains(&ViewFlag::ReferenceGrid) {
        // The canvas corners bound the grid; a rotated view needs all four.
        let corners = cam.world_corners();
        let lo = corners
            .iter()
            .fold(corners[0], |m, p| Point::new(m.x.min(p.x), m.y.min(p.y)));
        let hi = corners
            .iter()
            .fold(corners[0], |m, p| Point::new(m.x.max(p.x), m.y.max(p.y)));
        let minor = Stroke::new(1.0_f32, pal.grid_minor);
        let major = Stroke::new(1.0_f32, pal.grid_major);
        let (x0, x1) = (
            (lo.x / spacing).floor() as i64,
            (hi.x / spacing).ceil() as i64,
        );
        for k in x0..=x1 {
            let x = k as f64 * spacing;
            let stroke = if k % 5 == 0 { major } else { minor };
            painter.line_segment(
                [
                    cam.world_to_screen(Point::new(x, lo.y)),
                    cam.world_to_screen(Point::new(x, hi.y)),
                ],
                stroke,
            );
        }
        let (y0, y1) = (
            (lo.y / spacing).floor() as i64,
            (hi.y / spacing).ceil() as i64,
        );
        for k in y0..=y1 {
            let y = k as f64 * spacing;
            let stroke = if k % 5 == 0 { major } else { minor };
            painter.line_segment(
                [
                    cam.world_to_screen(Point::new(lo.x, y)),
                    cam.world_to_screen(Point::new(hi.x, y)),
                ],
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
            // A room with no floor under it (Open Below, R-40) is dashed.
            if crate::editor::rooms_edit::name_entry(cx, room).is_some_and(|n| !n.has_floor) {
                let mut pts = quad(cam, &room.polygon);
                if let Some(first) = pts.first().copied() {
                    pts.push(first);
                }
                painter.extend(Shape::dashed_line(
                    &pts,
                    Stroke::new(3.0_f32, pal.room_outline),
                    9.0,
                    6.0,
                ));
            }
        }
        if show_label {
            painter.text(
                cam.world_to_screen(crate::editor::rooms_edit::label_position(cx, room)),
                Align2::CENTER_CENTER,
                crate::editor::rooms_edit::room_label_text(cx, room),
                match face_of(
                    cx.project
                        .text_styles
                        .resolve(&crate::editor::rooms_edit::label_text_style(cx, room)),
                ) {
                    Some(spec) => crate::fonts::font_id(
                        painter.ctx(),
                        &spec,
                        crate::editor::rooms_edit::LABEL_FONT_PX as f32,
                    ),
                    None => FontId::proportional(crate::editor::rooms_edit::LABEL_FONT_PX as f32),
                },
                pal.room_label,
            );
        }
    }
    crate::editor::rooms_edit::draw_room_selection(cx, painter, cam);
}

/// The wall's drawn polygon: the mitered outline when the cache has one.
#[cfg_attr(not(test), allow(dead_code))]
fn wall_polygon(cx: &EditorContext, wall: &Wall) -> Vec<Point> {
    wall_polygon_at(cx, wall, 0.0)
}

/// How far an arc's facets may stray from the true curve on screen, pixels.
const ARC_SAG_PX: f64 = 0.4;

/// Facets to draw `wall`'s arc with at `px_per_in` pixels per inch: the
/// facet angle's count (W-65), more where a large radius at a high zoom
/// would show corners. `None` for a straight wall or an unknown zoom.
fn arc_facets(wall: &Wall, px_per_in: f64) -> Option<usize> {
    let curve = wall.curve.filter(|c| !c.is_straight())?;
    if px_per_in <= 0.0 {
        return None;
    }
    Some(curve.facet_count_for_sag(wall.start, wall.end, ARC_SAG_PX / px_per_in))
}

/// [`wall_polygon`] with an arc faceted for the zoom (`0.0`: the facet
/// angle's count).
fn wall_polygon_at(cx: &EditorContext, wall: &Wall, px_per_in: f64) -> Vec<Point> {
    // The join cache only knows straight walls: an arc gets its own miter.
    if wall.is_curved() {
        return plan_core::joins::curved_wall_polygon_n(
            &cx.floor().walls,
            wall.id,
            0.5,
            arc_facets(wall, px_per_in),
        )
        .unwrap_or_else(|| wall.plan_polygon());
    }
    outline_index(cx, wall.id)
        .and_then(|i| cx.outlines.get(i))
        .map_or_else(|| wall.footprint().to_vec(), |o| o.polygon.clone())
}

/// The context state `((uid, rev), outline count)` an outline index was built
/// from, and the index: wall id to position in `cx.outlines`.
type OutlineIndex = (((u64, u64), usize), HashMap<plan_core::Id, usize>);

thread_local! {
    static OUTLINE_INDEX: std::cell::RefCell<Option<OutlineIndex>> =
        const { std::cell::RefCell::new(None) };
}

/// The position of wall `id`'s outline in `cx.outlines` (a scan of the list
/// per wall made drawing quadratic in the number of walls).
fn outline_index(cx: &EditorContext, id: plan_core::Id) -> Option<usize> {
    OUTLINE_INDEX.with(|c| {
        let mut c = c.borrow_mut();
        let key = (cx.cache_key(), cx.outlines.len());
        if c.as_ref().is_none_or(|(k, _)| *k != key) {
            let mut map = HashMap::with_capacity(cx.outlines.len());
            for (i, o) in cx.outlines.iter().enumerate() {
                // The first outline of an id wins, like the scan it replaces.
                map.entry(o.wall_id).or_insert(i);
            }
            *c = Some((key, map));
        }
        c.as_ref().and_then(|(_, m)| m.get(&id).copied())
    })
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
        // A wall raised 48" or more off the floor is drawn dashed.
        if wall.bottom_offset >= plan_core::walls::ROOM_BOUNDARY_MAX_BOTTOM {
            weighted(cx, painter, &wall.layer, || {
                draw_raised_wall(cx, painter, cam, wall)
            });
            continue;
        }
        weighted(cx, painter, &wall.layer, || {
            draw_one_wall(cx, painter, cam, wall)
        });
    }
    // Walls left whole across each other draw as one shape (W-36).
    crate::editor::wall_edit::draw_crossing_merges(cx, painter, cam);
}

/// The wall's left and right face lines (a straight wall's footprint edges,
/// or the two offset curves of an arc), both running start to end.
fn face_lines(wall: &Wall) -> (Vec<Point>, Vec<Point>) {
    face_lines_at(wall, 0.0)
}

/// [`face_lines`] with an arc faceted for the zoom (`0.0`: the facet angle's
/// count).
fn face_lines_at(wall: &Wall, px_per_in: f64) -> (Vec<Point>, Vec<Point>) {
    if wall.is_curved() {
        match arc_facets(wall, px_per_in) {
            Some(n) => arc_faces(&wall.plan_polygon_n(n)),
            None => arc_faces(&wall.plan_polygon()),
        }
    } else {
        let f = wall.footprint();
        (vec![f[0], f[1]], vec![f[3], f[2]])
    }
}

/// The left and right face lines of an arc from its closed polygon (left
/// forward, then right back), as [`Wall::plan_polygon`] lays it out.
fn arc_faces(poly: &[Point]) -> (Vec<Point>, Vec<Point>) {
    let h = poly.len() / 2;
    (
        poly[..h].to_vec(),
        poly[h..].iter().rev().copied().collect(),
    )
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
        // The polygon carries the mitered ends.
        fill_band(painter, cam, poly, fill);
        painter.add(Shape::closed_line(quad(cam, poly), stroke));
    } else {
        painter.add(Shape::convex_polygon(quad(cam, poly), fill, stroke));
    }
}

/// Fills an arc band (left face forward, then right face back, as
/// [`Wall::plan_polygon`] lays it out) as a strip of convex quads.
fn fill_band(painter: &egui::Painter, cam: &Camera, poly: &[Point], fill: Color32) {
    let (left, right) = arc_faces(poly);
    for i in 0..left.len().saturating_sub(1) {
        let quad = [left[i], left[i + 1], right[i + 1], right[i]];
        painter.add(Shape::convex_polygon(
            quad.iter().map(|p| cam.world_to_screen(*p)).collect(),
            fill,
            Stroke::NONE,
        ));
    }
}

/// The layers of a curved wall as Chief draws a wall type in plan: every
/// layer boundary a thin line, the main layer filled darker with heavier
/// faces, all following the arc and mitered to the walls joined to it.
fn draw_curved_layers(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    wall: &Wall,
    fill: Color32,
) {
    let pal = &cx.palette;
    let mut layers: Vec<plan_core::joins::WallLayerOutline> = cx
        .layer_outlines
        .iter()
        .filter(|l| l.wall_id == wall.id)
        .cloned()
        .collect();
    // The cache is faceted by the facet angle: a finer arc for a high zoom.
    let cached = wall
        .curve
        .map_or(0, |c| c.facet_count(wall.start, wall.end));
    if let Some(n) = arc_facets(wall, cam.px_per_in).filter(|n| *n > cached.max(2)) {
        let types = if cx.project.wall_types.is_empty() {
            &cx.defaults.wall_types
        } else {
            &cx.project.wall_types
        };
        if let Some(fine) =
            plan_core::joins::curved_layer_outlines(&cx.floor().walls, types, wall.id, 0.5, Some(n))
        {
            layers = fine;
        }
    }
    let thin = Stroke::new(0.75_f32, pal.wall_stroke);
    let heavy = Stroke::new(1.5_f32, pal.wall_stroke);
    let main_fill = crate::theme::scale(fill, 0.78);
    for l in &layers {
        if l.is_main {
            fill_band(painter, cam, &l.polygon, main_fill);
        }
        painter.add(Shape::closed_line(quad(cam, &l.polygon), thin));
        if l.is_main {
            let (left, right) = arc_faces(&l.polygon);
            painter.add(Shape::line(quad(cam, &left), heavy));
            painter.add(Shape::line(quad(cam, &right), heavy));
        }
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

/// A wall that starts [`plan_core::walls::ROOM_BOUNDARY_MAX_BOTTOM`] or more
/// above the floor (a transom wall, a soffit, a bridge): it is overhead, so
/// the plan shows its outline dashed and unfilled, like Chief.
fn draw_raised_wall(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, wall: &Wall) {
    let mut pts = quad(cam, &wall_polygon_at(cx, wall, cam.px_per_in));
    if let Some(first) = pts.first().copied() {
        pts.push(first);
    }
    let stroke = Stroke::new(1.0_f32, cx.palette.wall_stroke.gamma_multiply(0.8));
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
    let (left, right) = face_lines_at(wall, cam.px_per_in);
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
        let color = if b.is_main {
            crate::theme::scale(fill, 0.78)
        } else {
            fill
        };
        if lower.is_curved() {
            // Each layer is a band between two offset arcs.
            let facets = arc_facets(&lower, cam.px_per_in).unwrap_or(2);
            let outer = lower.offset_curve(b.outer, facets);
            let inner = lower.offset_curve(b.inner, facets);
            let band: Vec<Point> = outer.into_iter().chain(inner.into_iter().rev()).collect();
            fill_band(painter, cam, &band, color);
            painter.add(Shape::closed_line(quad(cam, &band), thin));
            continue;
        }
        let pts: Vec<Pos2> = [
            wall.start.add(n.scale(b.outer)),
            wall.end.add(n.scale(b.outer)),
            wall.end.add(n.scale(b.inner)),
            wall.start.add(n.scale(b.inner)),
        ]
        .iter()
        .map(|p| cam.world_to_screen(*p))
        .collect();
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
    let poly = wall_polygon_at(cx, wall, cam.px_per_in);
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
        return draw_curved_layers(cx, painter, cam, wall, fill);
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
        // The Fill Style of this layer of the wall type (Layer Fill Style).
        if let Some(style) = wall
            .wall_type
            .as_deref()
            .and_then(|t| cx.project.wall_layer_fill(t, l.layer_index))
        {
            paint_style_fill(cx, painter, cam, &l.polygon, &[], style, &wall.layer);
        }
        painter.add(Shape::closed_line(pts.clone(), thin));
        if l.is_main && pts.len() == 4 {
            // Start-left to end-left and end-right to start-right.
            painter.line_segment([pts[0], pts[1]], heavy);
            painter.line_segment([pts[2], pts[3]], heavy);
        }
    }
}

/// Plan inches per paper inch of the active drawing scale.
fn plan_per_paper(cx: &EditorContext) -> f64 {
    let r = 12.0 / cx.sheet.scale.inches_per_foot();
    if r.is_finite() && r > 0.0 {
        r
    } else {
        48.0
    }
}

fn color_rgb(c: Color32) -> [u8; 3] {
    [c.r(), c.g(), c.b()]
}

/// Draws a Fill Style over `outer` minus `holes`; Use Layer takes the fill
/// style assigned to `layer` (nothing when it has none).
fn paint_style_fill(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    outer: &[Point],
    holes: &[Vec<Point>],
    style: &plan_core::fill_styles::FillStyle,
    layer: &str,
) {
    use plan_core::fill_styles::{FillStyle, FillTarget, PatternType};
    let layer_style = cx
        .project
        .styles
        .fill_for(&FillTarget::Layer(layer.to_string()))
        .cloned()
        .unwrap_or_else(FillStyle::use_layer);
    let style = style.resolved(&layer_style);
    if style.pattern == PatternType::UseLayer {
        return;
    }
    let layer_rgb = cx.layers().get(layer).map_or([0, 0, 0], |l| l.color);
    let k = if cx.view_flags.contains(&ViewFlag::LineWeights) {
        1.0
    } else {
        25.0 / style.line_weight.max(1) as f32
    };
    let patterns = cx.project.styles.all_patterns();
    crate::dialogs::fill_style::paint_fill(
        painter,
        &|p| cam.world_to_screen(p),
        cam.px_per_in as f32,
        outer,
        holes,
        style,
        &patterns,
        layer_rgb,
        color_rgb(cx.palette.background),
        k,
    );
}

/// The polygon a closed CAD item fills.
fn cad_fill_polygon(item: &CadItem) -> Option<Vec<Point>> {
    match item {
        CadItem::Polyline { points, closed: true } if points.len() >= 3 => Some(points.clone()),
        CadItem::Circle { .. } => crate::dialogs::line_style::item_path(item).map(|(p, _)| p),
        _ => None,
    }
}

/// A CAD object with a library line style or an assigned Fill Style. Draws
/// the fill (and, with a line style, the outline); returns true when the
/// object is completely drawn.
fn draw_styled_cad(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    c: &plan_core::CadObject,
    attrs: Option<&plan_core::cad::CadAttrs>,
) -> bool {
    use plan_core::fill_styles::FillTarget;
    let fill = cx.project.styles.fill_for(&FillTarget::Cad(c.id));
    let line = cx.project.assigned_line_style(c.id, &c.layer);
    if fill.is_none() && line.is_none() {
        return false;
    }
    if let (Some(style), Some(poly)) = (fill, cad_fill_polygon(&c.item)) {
        weighted(cx, painter, &c.layer, || {
            paint_style_fill(cx, painter, cam, &poly, &[], style, &c.layer)
        });
    }
    let Some(def) = line else { return false };
    // Arrow ends keep the legacy drawing.
    if attrs.is_some_and(|a| {
        a.arrow_start != plan_core::cad::ArrowStyle::None
            || a.arrow_end != plan_core::cad::ArrowStyle::None
    }) {
        return false;
    }
    let Some((pts, closed)) = crate::dialogs::line_style::item_path(&c.item) else {
        return false;
    };
    let color = attrs
        .and_then(|a| a.color)
        .map_or(cx.palette.text, |c| Color32::from_rgb(c[0], c[1], c[2]));
    // A legacy solid fill under the outline.
    if fill.is_none() {
        if let Some(f) = attrs
            .and_then(|a| a.fill.as_ref())
            .filter(|f| f.pattern.is_empty())
        {
            if let (Some(style), Some(poly)) = (
                plan_core::fill_styles::FillStyle::from_fill_attr(f),
                cad_fill_polygon(&c.item),
            ) {
                paint_style_fill(cx, painter, cam, &poly, &[], &style, &c.layer);
            }
        }
    }
    let scale = plan_per_paper(cx);
    weighted(cx, painter, &c.layer, || {
        crate::dialogs::line_style::paint_path(
            painter,
            cam,
            &def,
            &pts,
            closed,
            scale,
            Stroke::new(1.0_f32, color),
        )
    });
    true
}

/// Fill Styles assigned to slabs and rooms, over what those draw themselves.
fn draw_assigned_fills(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    use plan_core::fill_styles::FillTarget;
    let styles = &cx.project.styles;
    if styles.fill_assign.is_empty() {
        return;
    }
    let slabs = crate::dialogs::fill_style::slabs(cx);
    for (target, style) in &styles.fill_assign {
        match target {
            FillTarget::Slab(id) => {
                if let Some(s) = slabs.iter().find(|s| s.id == *id) {
                    if cx.layers().is_visible(&s.layer) {
                        paint_style_fill(cx, painter, cam, &s.outline, &s.holes, style, &s.layer);
                    }
                }
            }
            FillTarget::Room(anchor) => {
                if let Some(r) = cx
                    .rooms
                    .iter()
                    .find(|r| crate::editor::rooms_edit::room_anchor(r).dist(*anchor) < 1.0)
                {
                    if cx.layers().is_visible("Rooms") {
                        paint_style_fill(cx, painter, cam, &r.inner_polygon, &r.holes, style, "Rooms");
                    }
                }
            }
            _ => {}
        }
    }
}

/// Is poché on in the view that is showing (View > Poché)?
pub fn poche_on(cx: &EditorContext) -> bool {
    let on = cx
        .project
        .styles
        .poche
        .is_on(&cx.project.active_plan_view, plan_core::fill_styles::PocheView::Plan);
    crate::dialogs::fill_style::note_poche(on);
    on
}

/// Poché: the dark fill over the cut walls, for every layer of a wall that
/// has no Fill Style of its own. Not on glass walls, invisible walls, room
/// dividers, railings, fencing or raised walls (manual p. 226).
fn draw_poche(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    if !poche_on(cx) {
        return;
    }
    let pal = &cx.palette;
    let c = cx.project.styles.poche.color();
    let fill = Color32::from_rgb(c[0], c[1], c[2]);
    for wall in &cx.floor().walls {
        if !cx.layers().is_visible(&wall.layer) || !plan_core::fill_styles::wall_gets_poche(wall) {
            continue;
        }
        weighted(cx, painter, &wall.layer, || {
            let typed: Vec<&plan_core::joins::WallLayerOutline> = cx
                .layer_outlines
                .iter()
                .filter(|l| l.wall_id == wall.id)
                .collect();
            if typed.is_empty() || wall.is_curved() {
                let poly = wall_polygon_at(cx, wall, cam.px_per_in);
                fill_wall(painter, cam, wall, &poly, fill, Stroke::new(1.0_f32, pal.wall_stroke));
                return;
            }
            for l in typed {
                let styled = wall
                    .wall_type
                    .as_deref()
                    .and_then(|t| cx.project.wall_layer_fill(t, l.layer_index))
                    .is_some();
                if styled {
                    continue;
                }
                let pts = quad(cam, &l.polygon);
                painter.add(Shape::convex_polygon(
                    pts,
                    fill,
                    Stroke::new(1.0_f32, pal.wall_stroke),
                ));
            }
        });
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

/// The walls of the reference floor as plan polygons (View > Reference
/// Display, R-65): the floor below unless the Reference Display dialog chose
/// the floor above or another floor. Nothing when the display is off, when
/// there is no such floor, or for walls on layers the chosen layer set hides
/// or whose Ref box (LAY-10) is off.
#[cfg_attr(not(test), allow(dead_code))]
pub fn reference_polygons(cx: &EditorContext) -> Vec<Vec<Point>> {
    crate::dialogs::reference_display::reference_walls(cx)
        .iter()
        .map(|w| w.footprint().to_vec())
        .collect()
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
    let mut caption = cx.sheet.caption();
    let mode = sheet::preview_color_label(sheet::preview_color());
    if preview && !mode.is_empty() {
        caption = format!("{caption}  \u{b7}  {mode}");
    }
    painter.text(
        rect.left_top() + Vec2::new(6.0, 4.0),
        Align2::LEFT_TOP,
        caption,
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
        quad(cam, &wall_polygon_at(cx, wall, cam.px_per_in)),
        stroke,
    ));
}

/// A text object drawn at its printed size for the sheet's scale: a copy
/// with the height the object's text style gives at `cx.sheet.scale`, or
/// `None` when the style holds the plan height (or nothing changes).
pub fn printed_text_object(
    cx: &EditorContext,
    c: &plan_core::CadObject,
    attrs: Option<&plan_core::cad::CadAttrs>,
) -> Option<plan_core::CadObject> {
    let CadItem::Text { height, .. } = &c.item else {
        return None;
    };
    let h = cx.project.text_styles.drawn_height(
        cx.layers(),
        &c.layer,
        attrs.and_then(|a| a.text_style.as_deref()),
        *height,
        cx.sheet.scale.inches_per_foot(),
    );
    if (h - *height).abs() < 1e-9 {
        return None;
    }
    let mut o = c.clone();
    if let CadItem::Text { height, .. } = &mut o.item {
        *height = h;
    }
    Some(o)
}

/// A text with a box (TXT-1, TXT-3, TXT-16): background fill, wrapped lines
/// aligned inside the box, and the border. `c` is the text at its drawn
/// size (see [`printed_text_object`]). Lines are laid out by
/// [`plan_core::text_box`]; their runs are placed by the glyph widths of the
/// face in use so alignment is exact on screen.
pub fn draw_text_box(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    c: &plan_core::CadObject,
    attrs: &plan_core::cad::CadAttrs,
) {
    use plan_core::text_box::{BoxLayout, HAlign};
    let Some(pb) = plan_core::text_box::placed(&c.item, attrs) else {
        return;
    };
    let pal = &cx.palette;
    let px = cam.px_per_in as f32;
    let (pos, angle, tb) = (pb.pos, pb.angle, pb.tb);
    // Screen angle: plan angles run counter-clockwise, the screen's clockwise;
    // a rotated plan view turns the text with the plan.
    let screen_angle = angle + cam.rotation;
    let (sin_a, cos_a) = (screen_angle.sin() as f32, screen_angle.cos() as f32);
    let along = Vec2::new(cos_a, -sin_a);
    let sc = |x: f64, y: f64| cam.world_to_screen(BoxLayout::to_plan(pos, angle, Point::new(x, y)));
    // Glyphs first: their widths decide where each line starts and, for an
    // unwrapped box, how wide the box is.
    struct Piece {
        galley: std::sync::Arc<egui::Galley>,
        run: plan_core::text_styles::RichRun,
        color: Color32,
    }
    let base_px = (pb.text_height as f32 * px).clamp(6.0, 200.0);
    let lines: Vec<Vec<Piece>> = pb
        .layout
        .lines
        .iter()
        .map(|l| {
            l.runs
                .iter()
                .filter(|r| !r.text.is_empty())
                .map(|r| {
                    let color = r
                        .color
                        .map_or(pal.text, |k| Color32::from_rgb(k[0], k[1], k[2]));
                    let size = (base_px * r.scale as f32).clamp(6.0, 200.0);
                    // A run set in its own font family (Rich Text Edit Bar).
                    let font = match &r.font {
                        Some(f) => crate::fonts::font_id(
                            painter.ctx(),
                            &crate::fonts::spec_named(f, r.bold, r.italic),
                            size,
                        ),
                        None => text_font(painter, size),
                    };
                    Piece {
                        galley: painter.layout_no_wrap(r.text.clone(), font, color),
                        run: r.clone(),
                        color,
                    }
                })
                .collect()
        })
        .collect();
    let line_px: Vec<f32> = lines
        .iter()
        .map(|l| l.iter().map(|p| p.galley.size().x).sum())
        .collect();
    let box_w = if tb.width > 0.0 {
        tb.width
    } else {
        f64::from(line_px.iter().copied().fold(0.0_f32, f32::max)) / f64::from(px.max(1e-6))
    };
    let box_h = pb.layout.height;
    let grow = pb.grow();
    // Background, then the frame.
    let quad_pts = |g: f64| -> Vec<Pos2> {
        vec![
            sc(-g, -g),
            sc(box_w + g, -g),
            sc(box_w + g, box_h + g),
            sc(-g, box_h + g),
        ]
    };
    if let Some(k) = tb.background {
        painter.add(Shape::convex_polygon(
            quad_pts(grow),
            Color32::from_rgb(k[0], k[1], k[2]),
            Stroke::NONE,
        ));
    }
    if tb.border {
        let k = if cx.view_flags.contains(&ViewFlag::LineWeights) && tb.border_weight > 0 {
            restyle::weight_factor(tb.border_weight)
        } else {
            1.0
        };
        painter.add(Shape::closed_line(quad_pts(grow), Stroke::new(k, pal.text)));
    }
    for ((line, pieces), w_px) in pb.layout.lines.iter().zip(&lines).zip(&line_px) {
        let w_in = f64::from(*w_px) / f64::from(px.max(1e-6));
        let x0 = match tb.halign {
            HAlign::Left => 0.0,
            HAlign::Center => (box_w - w_in) * 0.5,
            HAlign::Right => box_w - w_in,
        };
        let mut x_px = 0.0_f32;
        for piece in pieces {
            let size = piece.galley.size();
            let h_in = f64::from(size.y) / f64::from(px.max(1e-6));
            // The galley's top-left, `h_in` above the line's bottom.
            let top_left = sc(
                x0 + f64::from(x_px) / f64::from(px.max(1e-6)),
                line.y + h_in,
            );
            let shape = egui::epaint::TextShape::new(top_left, piece.galley.clone(), piece.color)
                .with_angle(-screen_angle as f32);
            if piece.run.bold {
                let nudge = along * 0.7;
                let mut bold = shape.clone();
                bold.pos += nudge;
                painter.add(bold);
            }
            painter.add(shape);
            if piece.run.underline {
                let a = sc(x0 + f64::from(x_px) / f64::from(px.max(1e-6)), line.y)
                    - Vec2::new(-sin_a, -cos_a) * 1.0;
                let b = a + along * size.x;
                painter.line_segment([a, b], Stroke::new(1.0_f32, piece.color));
            }
            if piece.run.strike {
                let a = sc(
                    x0 + f64::from(x_px) / f64::from(px.max(1e-6)),
                    line.y + h_in * 0.35,
                );
                let b = a + along * size.x;
                painter.line_segment([a, b], Stroke::new(1.0_f32, piece.color));
            }
            x_px += size.x;
        }
    }
}

/// The outline of a text's box and frame, for the selection and hover
/// highlight.
fn highlight_text_box(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    c: &plan_core::CadObject,
    stroke: Stroke,
) -> bool {
    let attrs = cx.floor().cad_attrs(c.id);
    let Some(attrs) = attrs.filter(|a| a.text_box.needs_layout()) else {
        return false;
    };
    let drawn = printed_text_object(cx, c, Some(&attrs));
    let item = drawn.as_ref().map_or(&c.item, |o| &o.item);
    let Some(pb) = plan_core::text_box::placed(item, &attrs) else {
        return false;
    };
    let pts: Vec<Pos2> = pb
        .layout
        .corners(pb.pos, pb.angle, pb.grow())
        .iter()
        .map(|p| cam.world_to_screen(*p))
        .collect();
    painter.add(Shape::closed_line(pts, stroke));
    true
}

/// Settles the CAD text hits of `hits` (from the stored-height picking) with
/// the drawn size: a printed-size text found by its stored box but not by the
/// drawn one is dropped, one found only by the drawn box is added.
pub fn settle_text_hits(
    cx: &EditorContext,
    hits: Vec<ObjectRef>,
    p: Point,
    tol: f64,
) -> Vec<ObjectRef> {
    let floor = cx.floor();
    let attrs = floor.cad_attr_map();
    let mut drop = Vec::new();
    let mut add = Vec::new();
    for c in floor.cad.iter().rev() {
        // A text with a box is hit by its box and frame (TXT-1, S-25).
        let printed = printed_text_object(cx, c, attrs.get(&c.id));
        let boxed = attrs.get(&c.id).and_then(|a| {
            plan_core::text_box::placed(printed.as_ref().map_or(&c.item, |o| &o.item), a)
        });
        let Some(drawn) = printed.or_else(|| boxed.as_ref().map(|_| c.clone())) else {
            continue;
        };
        let r = ObjectRef::Cad(c.id);
        let found = hits.contains(&r);
        let dist = match &boxed {
            Some(pb) => pb.distance(p),
            None => crate::editor::selection::cad_distance(&drawn.item, p),
        };
        let now = cx.layers().is_visible(&c.layer) && dist <= tol;
        match (found, now) {
            (true, false) => drop.push(r),
            (false, true) => add.push(r),
            _ => {}
        }
    }
    let mut res: Vec<ObjectRef> = hits.into_iter().filter(|h| !drop.contains(h)).collect();
    res.extend(add);
    res
}

/// How big a dimension's text, arrows and extension lines are, in plan
/// inches: the dimension set's text style at the sheet's scale (DIM-7) and
/// its arrow and extension settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DimLook {
    /// Character height of the number.
    pub text_h: f64,
    /// Length of the end tick.
    pub arrow: f64,
    /// Gap between the measured point and its extension line.
    pub gap: f64,
    /// How far the extension line runs past the dimension line.
    pub past: f64,
    /// Number above the line (otherwise centered on it, over a break).
    pub above: bool,
    /// A fixed extension line length back from the dimension line.
    pub ext_length: Option<f64>,
    /// The end mark (Arrow tab) and whether an arrowhead or dot is solid.
    pub mark: plan_core::dimension::DimArrow,
    pub filled: bool,
    /// The leader line style of a moved label.
    pub leader: plan_core::dimension::LeaderStyle,
}

impl DimLook {
    /// The look of `d` under the active dimension defaults at the sheet's
    /// scale. A text style that holds its printed size (or a set that holds
    /// all its sizes on paper) gives the same size on paper at any scale;
    /// otherwise the plan sizes stay and print larger or smaller.
    pub fn of(cx: &EditorContext, d: &Dimension) -> DimLook {
        let set = &cx.defaults.dimensions;
        let ipf = cx.sheet.scale.inches_per_foot();
        let name = d
            .text_style
            .as_deref()
            .filter(|n| !n.is_empty())
            .or(Some(set.text_style.as_str()).filter(|n| !n.is_empty()))
            .unwrap_or("Dimension Text Style");
        let styles = &cx.project.text_styles;
        let text_h = styles
            .resolve(name)
            .or_else(|| cx.defaults.text_styles.resolve(name))
            .map_or(4.5, |st| st.plan_height_at(ipf, set.printed_size));
        // The set's lengths are plan inches at the 1/4" scale they were
        // captured for; a printed-size set keeps them on paper.
        let k = if set.printed_size && ipf > 0.0 {
            0.25 / ipf
        } else {
            1.0
        };
        let or = |v: f64, fallback: f64| if v > 0.0 { v } else { fallback };
        // What the dimension sets for itself wins (DIM-31, DIM-38).
        let o = &d.look;
        DimLook {
            text_h,
            arrow: o.arrow_size.unwrap_or(or(set.arrow_size, 2.25) * k),
            gap: o.ext_gap.unwrap_or(set.extension_gap.max(0.0) * k),
            past: o.ext_past.unwrap_or(set.extension_past.max(0.0) * k),
            above: set.text_above_line,
            ext_length: o.ext_length.filter(|l| *l > 0.0),
            mark: o
                .arrow
                .unwrap_or_else(|| plan_core::dimension::DimArrow::from_name(&set.arrow_style)),
            filled: o.arrow_filled.unwrap_or(true),
            leader: plan_core::dimension::LeaderStyle::from_name(&set.leader_style),
        }
    }

    /// The fixed 12 px text and 4 px ticks the tools' ghost previews use.
    fn screen(cam: &Camera) -> DimLook {
        let px = cam.px_per_in.max(1e-6);
        DimLook {
            text_h: 12.0 / px,
            arrow: 8.0 / px,
            gap: 0.0,
            past: 0.0,
            above: false,
            ext_length: None,
            mark: plan_core::dimension::DimArrow::Tick,
            filled: true,
            leader: plan_core::dimension::LeaderStyle::SquareCorner,
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
    let look = DimLook::screen(cam);
    draw_dimension_look(painter, cam, d, fmt, stroke, pal, &look);
}

/// The angle (radians, screen space) text along the line `a` to `b` is
/// drawn at, flipped so it never reads upside-down: vertical text reads
/// from bottom to top (DIM-8).
pub fn upright_angle(a: Pos2, b: Pos2) -> f32 {
    let mut ang = (b.y - a.y).atan2(b.x - a.x);
    let half_pi = std::f32::consts::FRAC_PI_2;
    if ang >= half_pi - 1e-4 {
        ang -= std::f32::consts::PI;
    } else if ang < -half_pi - 1e-4 {
        ang += std::f32::consts::PI;
    }
    ang
}

/// Draws a dimension with the sizes in `look` (see [`DimLook::of`]): the
/// extension lines, the dimension line (a straight line, or the arc of a
/// radius, arc length or angle), the end marks, the centerline marks and the
/// label laid out by [`Dimension::label_layout`] (above, centered or below
/// the line, a second format on the other side, moved or turned, with a
/// leader line, rich runs).
pub fn draw_dimension_look(
    painter: &egui::Painter,
    cam: &Camera,
    d: &Dimension,
    fmt: &plan_core::DimFormat,
    stroke: Stroke,
    pal: &Palette,
    look: &DimLook,
) {
    use plan_core::dimension::LabelParams;
    let px = cam.px_per_in as f32;
    let ext_stroke = Stroke::new(0.7_f32, stroke.color.gamma_multiply(0.8));
    let sc = |p: Point| cam.world_to_screen(p);
    let geom = d.curve_geom(look.gap, look.past);
    // Extension lines start `gap` off the measured point and run `past` the
    // dimension line; a point's can be switched off.
    let exts: Vec<(Point, Point)> = match &geom {
        Some(g) => g.extensions.clone(),
        None => d
            .extension_lines()
            .into_iter()
            .zip(d.hide_ext)
            .filter(|(_, hidden)| !hidden)
            .filter_map(|((m, e), _)| {
                plan_core::dimension::extension_segment(m, e, look.gap, look.past, look.ext_length)
            })
            .collect(),
    };
    for (from, to) in exts {
        painter.line_segment([sc(from), sc(to)], ext_stroke);
    }
    // The dimension line and where its ends point.
    let (line, fwd_start, fwd_end): (Vec<Point>, Point, Point) = match &geom {
        Some(g) => (g.line.clone(), g.inward[0], g.inward[1].scale(-1.0)),
        None => {
            let (p, q) = d.line_points();
            let u = q.sub(p).normalized();
            (vec![p, q], u, u)
        }
    };
    let screen_line: Vec<Pos2> = line.iter().map(|p| sc(*p)).collect();
    painter.add(Shape::line(screen_line.clone(), stroke));
    // A plan direction as a unit vector on screen.
    let dir_at = |at: Point, v: Point| -> Vec2 {
        let (a, b) = (sc(at), sc(at.add(v.scale(1.0))));
        let s = b - a;
        if s.length() > 1e-6 {
            s / s.length()
        } else {
            Vec2::X
        }
    };
    if let (Some(first), Some(last)) = (line.first(), line.last()) {
        draw_dimension_end(painter, sc(*first), dir_at(*first, fwd_start), 1.0, look, px, stroke);
        draw_dimension_end(painter, sc(*last), dir_at(*last, fwd_end), -1.0, look, px, stroke);
    }
    // Centerline marks on the extension lines (Extensions panel).
    for (k, on) in d.look.seg.centerline.iter().enumerate() {
        if !*on {
            continue;
        }
        let (m, e) = d.extension_lines()[k];
        let out = e.add(e.sub(m).normalized().scale(look.text_h));
        let font = text_font(painter, (look.text_h as f32 * px * 0.8).clamp(6.0, 120.0));
        painter.text(sc(out), Align2::CENTER_CENTER, "CL", font, pal.dimension_text);
    }

    // The label.
    let (anchor, dirv, run, len) = match &geom {
        Some(g) => (g.label_at, g.label_dir, None, f64::INFINITY),
        None => {
            let (p, q) = d.line_points();
            (Point::lerp(p, q, 0.5), q.sub(p).normalized(), Some((p, q)), p.dist(q))
        }
    };
    let font_px = (look.text_h as f32 * px).clamp(6.0, 200.0);
    let width = |t: &str| {
        painter
            .layout_no_wrap(t.to_string(), text_font(painter, font_px), pal.dimension_text)
            .size()
            .x as f64
            / cam.px_per_in.max(1e-9)
    };
    let params = LabelParams {
        text_h: look.text_h,
        width: &width,
        view_rotation: cam.rotation,
        leader: look.leader,
    };
    let lay = d.label_layout(fmt, anchor, dirv, run, len, &params);
    if let Some(k) = lay.knockout {
        painter.add(Shape::convex_polygon(
            k.iter().map(|p| sc(*p)).collect(),
            pal.background.gamma_multiply(0.85),
            Stroke::NONE,
        ));
    }
    if lay.leader.len() >= 2 {
        painter.add(Shape::line(
            lay.leader.iter().map(|p| sc(*p)).collect(),
            Stroke::new(0.7_f32, stroke.color),
        ));
    }
    if let Some((a, b)) = lay.stub {
        painter.line_segment([sc(a), sc(b)], Stroke::new(0.7_f32, stroke.color));
    }
    // Screen angle of the text: y runs down on screen and the view may be
    // turned.
    let ang = -((lay.angle + cam.rotation) as f32);
    let (sin, cos) = ang.sin_cos();
    let rich = d.look.seg.runs.clone();
    for (i, line) in lay.lines.iter().enumerate() {
        let galley = if i == 0 && !rich.is_empty() {
            let mut job = egui::text::LayoutJob::default();
            for r in &rich {
                job.append(
                    &r.text,
                    0.0,
                    egui::text::TextFormat {
                        font_id: text_font(painter, (font_px * r.scale as f32).clamp(4.0, 400.0)),
                        color: r
                            .color
                            .map_or(pal.dimension_text, |c| Color32::from_rgb(c[0], c[1], c[2])),
                        italics: r.italic,
                        underline: if r.underline {
                            Stroke::new(1.0_f32, pal.dimension_text)
                        } else {
                            Stroke::NONE
                        },
                        strikethrough: if r.strike {
                            Stroke::new(1.0_f32, pal.dimension_text)
                        } else {
                            Stroke::NONE
                        },
                        ..Default::default()
                    },
                );
            }
            painter.layout_job(job)
        } else {
            painter.layout_no_wrap(
                line.text.clone(),
                text_font(painter, font_px),
                pal.dimension_text,
            )
        };
        let c = sc(line.center);
        let half = galley.size() * 0.5;
        let top_left = c - Vec2::new(half.x * cos - half.y * sin, half.x * sin + half.y * cos);
        painter.add(
            egui::epaint::TextShape::new(top_left, galley, pal.dimension_text).with_angle(ang),
        );
    }
}

/// One end mark of a dimension line at `at` (screen), `dir` the unit
/// direction the line runs in there (from its first end to its last) and
/// `sign` `1.0` for the first end and `-1.0` for the last: a tick across the
/// line, a slash, an arrowhead pointing outward, a dot or nothing (the Arrow
/// tab).
fn draw_dimension_end(
    painter: &egui::Painter,
    at: Pos2,
    dir: Vec2,
    sign: f32,
    look: &DimLook,
    px: f32,
    stroke: Stroke,
) {
    use plan_core::dimension::DimArrow;
    let perp = Vec2::new(-dir.y, dir.x);
    let size = look.arrow as f32 * px;
    match look.mark {
        DimArrow::None => {}
        DimArrow::Tick => {
            let tick = perp * (size * 0.5).clamp(2.5, 14.0);
            painter.line_segment([at - tick, at + tick], stroke);
        }
        DimArrow::Slash => {
            // A long drafting slash, steeper than the tick (about 60 degrees).
            let len = (size * 0.6).clamp(4.0, 20.0);
            let slash = perp * len + dir * (len * 0.58);
            painter.line_segment([at - slash, at + slash], stroke);
        }
        DimArrow::Dot => {
            let r = (size * 0.25).clamp(1.5, 6.0);
            if look.filled {
                painter.circle_filled(at, r, stroke.color);
            } else {
                painter.circle_stroke(at, r, stroke);
            }
        }
        DimArrow::Arrow => {
            let len = size.clamp(5.0, 30.0);
            let half = len * 0.3;
            // The tip is at the end of the line; the base lies along it.
            let base = at + dir * (sign * len);
            if look.filled {
                painter.add(Shape::convex_polygon(
                    vec![at, base + perp * half, base - perp * half],
                    stroke.color,
                    Stroke::NONE,
                ));
            } else {
                painter.line_segment([at, base + perp * half], stroke);
                painter.line_segment([at, base - perp * half], stroke);
            }
        }
    }
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
                text_font(
                    painter,
                    ((*height * cam.px_per_in) as f32).clamp(6.0, 200.0),
                ),
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
            // The outline follows the arc on a curved wall (DW-88).
            let reach = w.thickness * 0.5 + 1.0;
            painter.add(Shape::closed_line(
                quad(
                    cam,
                    &w.band(op.start_offset(), op.end_offset(), -reach, reach),
                ),
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
                if !highlight_text_box(cx, painter, cam, c, stroke) {
                    draw_cad(painter, cam, &c.item, stroke, &cx.palette);
                }
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
        // Compound 3D solids and the box of an architectural block.
        ObjectRef::Solid(_) | ObjectRef::Block(_) => {
            crate::editor::solids_view::highlight(cx, painter, cam, o, stroke)
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
    use plan_core::OpeningKind;

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

    #[test]
    fn reference_display_shows_the_chosen_floor() {
        use crate::dialogs::reference_display::{
            reset_settings, set_settings, ReferenceFloor, ReferenceSettings,
        };
        reset_settings();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project.build_new_floor(false);
        cx.project.build_new_floor(false);
        // One wall on the ground floor, two on the third.
        let wall = |cx: &mut EditorContext, f: usize, x: f64| {
            cx.project.add_wall(
                f,
                Point::new(x, 0.0),
                Point::new(x + 120.0, 0.0),
                6.0,
                100.0,
                WallKind::Exterior,
            );
        };
        wall(&mut cx, 0, 0.0);
        wall(&mut cx, 2, 0.0);
        wall(&mut cx, 2, 200.0);
        cx.view_flags.insert(ViewFlag::ReferenceDisplay);
        cx.floor = 1;
        assert!(
            reference_polygons(&cx).len() == 1,
            "the floor below by default"
        );
        let mut s = ReferenceSettings {
            floor: ReferenceFloor::Above,
            ..ReferenceSettings::default()
        };
        set_settings(s.clone());
        assert_eq!(reference_polygons(&cx).len(), 2, "the floor above");
        cx.floor = 2;
        assert!(
            reference_polygons(&cx).is_empty(),
            "nothing above the top floor"
        );
        s.floor = ReferenceFloor::Floor(0);
        set_settings(s.clone());
        assert_eq!(
            reference_polygons(&cx).len(),
            1,
            "any floor, here the first"
        );
        cx.floor = 0;
        assert!(
            reference_polygons(&cx).is_empty(),
            "a floor is not its own reference"
        );
        // A layer set that hides the walls' layer hides them in the reference.
        cx.floor = 2;
        let layer = cx.project.floors[0].walls[0].layer.clone();
        let mut hidden = plan_core::layer_sets::LayerSetDef::new("No walls");
        hidden.ensure_state(&layer).display = false;
        cx.project.layer_sets.add_set(hidden);
        s.layer_set = Some("No walls".into());
        set_settings(s);
        assert!(reference_polygons(&cx).is_empty());
        reset_settings();
    }

    /// A plan with a nested room, fills in every pattern, an Open Below room
    /// and a dragged label draws, and the fill leaves the island open.
    #[test]
    fn nested_rooms_open_below_and_moved_labels_draw() {
        use crate::editor::rooms_edit::{self, FillPattern, FillStyle};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let box_ = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        let closet = [
            Point::new(60.0, 60.0),
            Point::new(120.0, 60.0),
            Point::new(120.0, 96.0),
            Point::new(60.0, 96.0),
        ];
        for (ring, kind) in [(box_, WallKind::Exterior), (closet, WallKind::Interior)] {
            for i in 0..4 {
                cx.project
                    .add_wall(0, ring[i], ring[(i + 1) % 4], 6.0, 100.0, kind);
            }
        }
        cx.refresh();
        let outer = cx.rooms.iter().position(|r| !r.holes.is_empty()).unwrap();
        let mut extras = rooms_edit::extras_for(&cx, &cx.rooms[outer].clone());
        let mut draft = plan_core::RoomName::new(
            rooms_edit::room_anchor(&cx.rooms[outer]),
            "Great Room",
            "Open Below",
        );
        draft.has_floor = false;
        for pattern in [FillPattern::Solid, FillPattern::Hatch, FillPattern::Grid] {
            extras.fill = FillStyle {
                pattern,
                ..FillStyle::default()
            };
            assert!(rooms_edit::apply_room_spec(&mut cx, outer, &draft, &extras));
            assert!(rooms_edit::set_label_offset(
                &mut cx,
                outer,
                Point::new(30.0, 20.0)
            ));
            let egui_ctx = egui::Context::default();
            let mut shapes = 0;
            let _ = egui_ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (_, painter) =
                        ui.allocate_painter(Vec2::new(400.0, 300.0), egui::Sense::hover());
                    let mut cam = Camera::default_view();
                    cam.rect = painter.clip_rect();
                    draw_plan(&cx, &painter, &cam);
                    painter.for_each_shape(|_| shapes += 1);
                });
            });
            assert!(shapes > 0);
        }
        // The hatch skips the island: no segment starts inside it.
        let hole = &cx.rooms[outer].holes[0];
        let inner = &cx.rooms[outer].inner_polygon;
        let n = Point::new(1.0, 0.0);
        for (a, b) in rooms_edit::hatch_lines(inner, &cx.rooms[outer].holes, n, 12.0) {
            let mid = Point::lerp(a, b, 0.5);
            assert!(!plan_core::geometry::point_in_polygon(mid, hole));
        }
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

    #[test]
    fn print_preview_shows_the_colour_mode_of_the_print_dialog() {
        use plan_layout::PrintColor;
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
        // The colours of the lines the plan draws.
        let line_colors = |cx: &EditorContext| {
            let mut out = Vec::new();
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
                            if stroke.color.a() > 0 {
                                out.push(stroke.color);
                            }
                        }
                    });
                });
            });
            out
        };
        let is_gray = |c: &Color32| c.r() == c.g() && c.g() == c.b();
        let is_bw = |c: &Color32| is_gray(c) && (c.r() == 0 || c.r() == 255);
        // Without Print Preview the mode is not applied.
        sheet::set_preview_color(PrintColor::BlackWhite);
        assert!(!line_colors(&cx).iter().all(is_gray));
        cx.view_flags.insert(ViewFlag::PrintPreview);
        cx.view_flags.insert(ViewFlag::DrawingSheet);
        let bw = line_colors(&cx);
        assert!(!bw.is_empty() && bw.iter().all(is_bw), "{bw:?}");
        sheet::set_preview_color(PrintColor::Grayscale);
        let g = line_colors(&cx);
        assert!(g.iter().all(is_gray) && !g.iter().all(is_bw), "{g:?}");
        sheet::set_preview_color(PrintColor::Color);
        assert!(!line_colors(&cx).iter().all(is_gray));
        // The caption names the mode.
        assert_eq!(
            sheet::preview_color_label(PrintColor::Grayscale),
            "Grayscale"
        );
        assert_eq!(sheet::preview_color_label(PrintColor::Color), "");
    }

    /// Plan shapes of one 240" wall of `class`: (filled paths, line segments).
    fn wall_shapes(class: WallClass, curved: bool) -> (usize, usize) {
        wall_shapes_raised(class, curved, 0.0)
    }

    /// [`wall_shapes`] for a wall starting `bottom` inches above the floor.
    fn wall_shapes_raised(class: WallClass, curved: bool, bottom: f64) -> (usize, usize) {
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
        w.bottom_offset = bottom;
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
    fn a_wall_raised_48_inches_or_more_is_drawn_dashed_and_unfilled() {
        let (fill, lines) = wall_shapes_raised(WallClass::Standard, false, 0.0);
        assert!(fill >= 1 && lines > 0);
        // Just under 48" is still an ordinary wall.
        let (fill, _) = wall_shapes_raised(WallClass::Standard, false, 47.0);
        assert!(fill >= 1);
        // From 48" up: no fill, and the outline is many short dashes.
        let (fill, lines) = wall_shapes_raised(WallClass::Standard, false, 48.0);
        assert_eq!(fill, 0);
        assert!(lines > 10, "{lines} dashes");
        // Curved walls too.
        let (fill, lines) = wall_shapes_raised(WallClass::Standard, true, 84.0);
        assert_eq!(fill, 0);
        assert!(lines > 10);
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

    #[test]
    fn arcs_get_more_facets_at_a_high_zoom_and_their_layers_follow_the_arc() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_wall(
            0,
            Point::ZERO,
            Point::new(480.0, 0.0),
            7.625,
            109.0,
            WallKind::Exterior,
        );
        {
            let w = cx.project.floors[0].wall_mut(id).unwrap();
            w.wall_type = Some("Stucco-6".into());
            w.curve = plan_core::WallCurve::from_radius(480.0, 400.0, true);
        }
        cx.refresh();
        let w = cx.floor().wall(id).unwrap().clone();
        let base = w.curve.unwrap().facet_count(w.start, w.end);
        // Zoomed out the facet angle's count is plenty; zoomed in it is not.
        assert_eq!(arc_facets(&w, 0.2), Some(base));
        let high = arc_facets(&w, 12.0).unwrap();
        assert!(high > base * 2, "{high} facets vs {base}");
        assert_eq!(wall_polygon_at(&cx, &w, 12.0).len(), 2 * (high + 1));
        assert_eq!(wall_polygon_at(&cx, &w, 0.0).len(), 2 * (base + 1));
        // The layers are arcs too: one polygon per layer, all on the arc.
        let layers: Vec<_> = cx
            .layer_outlines
            .iter()
            .filter(|l| l.wall_id == id)
            .collect();
        assert!(layers.len() > 1);
        let (c, r) = w.arc_center_radius().unwrap();
        for l in &layers {
            assert_eq!(l.polygon.len(), 2 * (base + 1), "{}", l.name);
            for p in &l.polygon {
                let off = (p.dist(c) - r).abs();
                assert!(off <= 3.8125 + 1e-6, "{} is {off} off the arc", l.name);
            }
        }
        // Drawing the arc works at any zoom; with the layers shown (from
        // 1 px/in) it is drawn in more pieces than the zoomed-out single fill.
        let egui_ctx = egui::Context::default();
        let vertices = |zoom: f64| {
            let out = egui_ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (_, painter) =
                        ui.allocate_painter(Vec2::new(800.0, 600.0), egui::Sense::hover());
                    let mut cam = Camera::default_view();
                    cam.rect = painter.clip_rect();
                    cam.center = Point::new(240.0, 80.0);
                    cam.px_per_in = zoom;
                    draw_plan(&cx, &painter, &cam);
                });
            });
            let meshes = egui_ctx.tessellate(out.shapes, out.pixels_per_point);
            meshes
                .iter()
                .map(|m| match &m.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => mesh.vertices.len(),
                    _ => 0,
                })
                .sum::<usize>()
        };
        for zoom in [0.2, 1.0, 3.0, 12.0] {
            assert!(vertices(zoom) > 0, "nothing drawn at {zoom}");
        }
        assert!(vertices(1.0) > vertices(0.2));
    }

    #[test]
    fn printed_size_text_and_dimension_numbers_follow_the_sheet_scale() {
        use plan_docs::Scale;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project.add_cad(
            0,
            "Text",
            CadItem::Text {
                pos: Point::ZERO,
                text: "KITCHEN".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        let obj = cx.floor().cad[0].clone();
        let height_of = |o: &plan_core::CadObject| match &o.item {
            CadItem::Text { height, .. } => *height,
            _ => 0.0,
        };
        // A character-height style leaves the plan height alone.
        cx.sheet.scale = Scale::EighthInch;
        assert!(printed_text_object(&cx, &obj, None).is_none());
        // A printed-size style: 1/8" on paper is 6" at 1/4", 12" at 1/8".
        let i = cx
            .project
            .text_styles
            .styles
            .iter()
            .position(|s| s.name == "Default Text Style")
            .unwrap();
        cx.project.text_styles.styles[i].use_printed_size(true);
        for (scale, plan_h) in [
            (Scale::QuarterInch, 6.0),
            (Scale::EighthInch, 12.0),
            (Scale::HalfInch, 3.0),
        ] {
            cx.sheet.scale = scale;
            let h = printed_text_object(&cx, &obj, None).map_or(height_of(&obj), |o| height_of(&o));
            assert!((h - plan_h).abs() < 1e-9, "{scale:?}: {h}");
            // The same 1/8" on paper.
            assert!((h * scale.inches_per_foot() / 12.0 - 0.125).abs() < 1e-9);
        }
        // Dimension numbers: the Dimension Text Style (4.5" plan) until the
        // set or the style holds the printed size.
        let d = Dimension::new(
            1,
            DimensionKind::Manual,
            Point::ZERO,
            Point::new(100.0, 0.0),
            24.0,
        );
        cx.sheet.scale = Scale::EighthInch;
        assert!((DimLook::of(&cx, &d).text_h - 4.5).abs() < 1e-9);
        cx.defaults.dimensions.printed_size = true;
        let look = DimLook::of(&cx, &d);
        assert!((look.text_h - 9.0).abs() < 1e-9, "{look:?}");
        assert!((look.arrow - cx.defaults.dimensions.arrow_size * 2.0).abs() < 1e-9);
        // Draws (rotated, outside and above) without panicking.
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let painter = ui.painter().clone();
                let cam = Camera::default_view();
                let fmt = cx.dim_format();
                let mut short = d.clone();
                short.end = Point::new(6.0, 0.0);
                let mut vertical = d.clone();
                vertical.end = Point::new(0.0, 100.0);
                for dim in [&d, &short, &vertical] {
                    let look = DimLook::of(&cx, dim);
                    draw_dimension_look(
                        &painter,
                        &cam,
                        dim,
                        &fmt,
                        Stroke::new(1.0_f32, Color32::BLACK),
                        &cx.palette,
                        &look,
                    );
                }
            });
        });
    }

    #[test]
    fn dimension_text_reads_upright() {
        let o = Pos2::new(0.0, 0.0);
        let ang = |x: f32, y: f32| upright_angle(o, Pos2::new(x, y)).to_degrees();
        assert!(ang(10.0, 0.0).abs() < 1e-3);
        // Right to left flips to read left to right.
        assert!(ang(-10.0, 0.0).abs() < 1e-3);
        // Vertical text reads from bottom to top, whichever way it is drawn.
        assert!((ang(0.0, 10.0) + 90.0).abs() < 1e-3);
        assert!((ang(0.0, -10.0) + 90.0).abs() < 1e-3);
        // Never past 90 degrees either way.
        for (x, y) in [(3.0, 8.0), (-3.0, 8.0), (3.0, -8.0), (-3.0, -8.0)] {
            let a = ang(x, y);
            assert!((-90.0..90.0).contains(&a), "{a}");
        }
    }
}

/// Performance benchmark on the large sample (`cargo test -p plan-app perf_bench -- --ignored --nocapture`).
#[cfg(test)]
#[path = "perf_bench.rs"]
mod perf_bench;
