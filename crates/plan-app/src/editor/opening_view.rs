//! Doors and windows in the plan: the symbol of each flavor (DW-38..DW-49)
//! drawn from [`plan_core::opening_symbol`], and the labels over the openings
//! (DW-59..DW-63): the size (`3068` or `2'-6" x 6'-8"`) or, when a schedule
//! numbers the opening, its mark (`D01`), in the label text style.

use super::schedule_view;
use super::{Camera, EditorContext};
use crate::theme::Palette;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use plan_core::geometry::Point;
use plan_core::opening_symbol::{plan_symbol_in, PartKind, SymbolPart};
use plan_core::{exterior_sign, Floor, Id, LabelPlacement, Opening, OpeningKind, Wall};
use plan_docs::schedule_kinds::Callout;
use std::collections::HashMap;

/// The colour of openings on a Window Level other than 0.
const LEVEL_GREY: Color32 = Color32::from_gray(170);
/// Smallest label text drawn, in screen pixels.
const MIN_LABEL_PX: f32 = 3.0;
/// Dash and gap of hidden lines, in screen pixels.
const DASH_PX: (f32, f32) = (5.0, 3.0);
/// Average character width as a fraction of the text height.
const CHAR_W: f64 = 0.56;

fn screen(cam: &Camera, pts: &[Point]) -> Vec<Pos2> {
    pts.iter().map(|p| cam.world_to_screen(*p)).collect()
}

/// One part of a symbol with the pen its kind calls for.
fn draw_part(
    painter: &egui::Painter,
    cam: &Camera,
    part: &SymbolPart,
    line_col: Color32,
    arc_col: Color32,
) {
    let mut pts = screen(cam, &part.points);
    if pts.len() < 2 {
        return;
    }
    let thin = Stroke::new(0.8_f32, line_col);
    let stroke = match part.kind {
        PartKind::Jamb | PartKind::Leaf | PartKind::Arrow | PartKind::Sill => {
            Stroke::new(1.0_f32, line_col)
        }
        PartKind::Swing | PartKind::Indicator => Stroke::new(1.0_f32, arc_col),
        PartKind::Frame
        | PartKind::Glass
        | PartKind::Hidden
        | PartKind::Track
        | PartKind::Threshold => thin,
    };
    match part.kind {
        PartKind::Hidden | PartKind::Track => {
            if part.closed {
                pts.push(pts[0]);
            }
            painter.extend(Shape::dashed_line(&pts, stroke, DASH_PX.0, DASH_PX.1));
        }
        _ if part.closed => {
            painter.add(Shape::closed_line(pts, stroke));
        }
        _ => {
            painter.add(Shape::line(pts, stroke));
        }
    }
}

/// An opening in its wall: the wall is cut, then the symbol of its flavor is
/// drawn (jambs, leaf and swing arc, window lines, pocket, track, projecting
/// unit). `exterior` is the wall side of the outside (see
/// [`plan_core::exterior_sign`]). `ghost` draws a placement preview.
pub fn draw_opening(
    painter: &egui::Painter,
    cam: &Camera,
    wall: &Wall,
    o: &Opening,
    pal: &Palette,
    exterior: f64,
    ghost: bool,
) {
    draw_opening_in(painter, cam, wall, o, pal, exterior, ghost, false);
}

/// A ghost of an opening that stands over another one (the transom a window
/// clicked onto a door would become): dashed, and the wall is not cleared
/// again.
pub fn draw_opening_over(
    painter: &egui::Painter,
    cam: &Camera,
    wall: &Wall,
    o: &Opening,
    pal: &Palette,
    exterior: f64,
) {
    draw_opening_in(painter, cam, wall, o, pal, exterior, true, true);
}

/// [`draw_opening`] for an opening of `floor`: one that stands over another
/// opening of its wall (a transom over a door) is drawn dashed, as what is
/// above the plan's cut plane, and does not clear the wall again.
pub fn draw_floor_opening(
    painter: &egui::Painter,
    cam: &Camera,
    floor: &Floor,
    wall: &Wall,
    o: &Opening,
    pal: &Palette,
    exterior: f64,
) {
    let over = floor
        .openings_on(wall.id)
        .any(|v| plan_core::openings::stands_over(o, v));
    draw_opening_in(painter, cam, wall, o, pal, exterior, false, over);
}

#[allow(clippy::too_many_arguments)]
fn draw_opening_in(
    painter: &egui::Painter,
    cam: &Camera,
    wall: &Wall,
    o: &Opening,
    pal: &Palette,
    exterior: f64,
    ghost: bool,
    over: bool,
) {
    let (line_col, arc_col) = if ghost {
        (pal.ghost_stroke, pal.ghost_stroke)
    } else if o.extras.spec.level != 0 {
        // Window Levels other than 0 draw light grey (manual p. 612).
        (LEVEL_GREY, LEVEL_GREY)
    } else {
        (pal.opening_line, pal.door_arc)
    };
    let sym = plan_symbol_in(wall, o, exterior, over);
    let half = wall.thickness * 0.5;
    let reach = half + 1.0 + 1.0 / cam.px_per_in;
    // A canvas-colored band hides the wall fill and its stroke across the
    // opening (along the arc on a curved wall); a niche only clears the band
    // it is cut into.
    let lo = if sym.cut.0 <= -half + 1e-9 {
        -reach
    } else {
        sym.cut.0
    };
    let hi = if sym.cut.1 >= half - 1e-9 {
        reach
    } else {
        sym.cut.1
    };
    if !over {
        for quad in wall.band_quads(sym.span.0, sym.span.1, lo, hi) {
            painter.add(Shape::convex_polygon(
                screen(cam, &quad),
                pal.background,
                Stroke::NONE,
            ));
        }
    }
    for part in &sym.parts {
        draw_part(painter, cam, part, line_col, arc_col);
    }
}

/// A label to put over an opening.
#[derive(Debug, Clone, PartialEq)]
pub struct OpeningLabel {
    pub opening: Id,
    pub kind: OpeningKind,
    pub text: String,
    /// The text is the schedule mark (drawn in its bubble).
    pub is_mark: bool,
    /// Where the center of the text goes, plan inches.
    pub at: Point,
}

/// The marks of the doors and windows a schedule numbers (`D01`, `W03`).
fn marks(cx: &EditorContext) -> HashMap<Id, String> {
    schedule_view::labels(cx)
        .into_iter()
        .filter(|c| {
            matches!(
                c.kind,
                plan_core::schedules::ScheduleKind::Door
                    | plan_core::schedules::ScheduleKind::Window
            )
        })
        .map(|c| (c.object, c.text))
        .collect()
}

/// Where the center of a label of `text_w` x `text_h` inches goes: on the
/// wall for `Center`, else beside it on the room side (`Interior`) or the
/// outside (`Exterior`), clear of the wall face.
pub fn label_anchor(
    wall: &Wall,
    o: &Opening,
    placement: LabelPlacement,
    exterior: f64,
    text_w: f64,
    text_h: f64,
) -> Point {
    let c = wall.point_along(o.center_offset);
    let side = match placement {
        LabelPlacement::Center => return c,
        LabelPlacement::Interior => -exterior,
        LabelPlacement::Exterior => exterior,
    };
    let n = wall.normal_along(o.center_offset);
    let reach = wall.thickness * 0.5 + 1.5 + n.x.abs() * text_w * 0.5 + n.y.abs() * text_h * 0.5;
    c.add(n.scale(side * reach))
}

/// Rough width of `text` set `h` inches high.
fn text_width(text: &str, h: f64) -> f64 {
    text.chars().count() as f64 * CHAR_W * h
}

/// The opening whose label stands for `o`: `o` itself when it is not in a
/// mulled unit or the unit shows component labels; for a unit with one label,
/// the first component stretched over the whole unit (the other components
/// have none); `None` when the unit suppresses its labels.
pub fn unit_label_opening(cx: &EditorContext, o: &Opening) -> Option<Opening> {
    use plan_core::openings::mull::MulledLabel;
    let Some(spec) = o.extras.spec.mulled.as_ref() else {
        return Some(o.clone());
    };
    match spec.label {
        MulledLabel::Components => Some(o.clone()),
        MulledLabel::Suppress => None,
        MulledLabel::Single => {
            let members = cx.project.unit_components(cx.floor, o.id);
            if members.first() != Some(&o.id) {
                return None;
            }
            let hole = cx.project.unit_hole(cx.floor, o.id)?;
            let mut u = o.clone();
            u.center_offset = (hole.s0 + hole.s1) * 0.5;
            u.width = hole.s1 - hole.s0;
            u.sill_height = hole.h0;
            u.height = hole.h1 - hole.h0;
            Some(u)
        }
    }
}

/// The text and the unmoved anchor of the label of `o` (before its dragged
/// offset), or `None` when the label is suppressed or hidden in plan.
fn label_base(
    cx: &EditorContext,
    o: &Opening,
    wall: &Wall,
    marks: &HashMap<Id, String>,
    h: f64,
) -> Option<(String, Option<String>, Point)> {
    let mark = marks.get(&o.id).map(String::as_str);
    // A mulled unit shows one label for all of it, the labels of its
    // components, or none (Mulled Unit Specification, Label panel).
    let o = &unit_label_opening(cx, o)?;
    let text = o.plan_label(&cx.defaults.opening_labels, mark)?;
    let placement = o.label_settings(&cx.defaults.opening_labels).placement;
    let ext = exterior_sign(wall, &cx.rooms);
    let at = label_anchor(wall, o, placement, ext, text_width(&text, h), h);
    Some((text, mark.map(str::to_string), at))
}

/// Where the dragged offset (along the wall, across it) puts a label.
pub fn apply_label_offset(wall: &Wall, at: Point, offset: (f64, f64)) -> Point {
    let s = wall.locate(at).0;
    at.add(wall.tangent_along(s).scale(offset.0))
        .add(wall.normal_along(s).scale(offset.1))
}

/// Whether the labels of openings of `kind` are shown: both the object layer
/// and its label layer ("Doors, Labels" / "Windows, Labels") are visible.
pub fn labels_visible(cx: &EditorContext, kind: OpeningKind) -> bool {
    let layer = match kind {
        OpeningKind::Door => "Doors",
        OpeningKind::Window => "Windows",
    };
    cx.layers().is_visible(layer)
        && cx
            .layers()
            .is_visible(plan_core::LayerSet::label_layer_of(kind))
}

/// The labels to draw on the active floor: one per door or window whose
/// layers are visible and whose label is not suppressed. An opening a
/// schedule numbers shows its mark, any other its size (or custom text). A
/// label dragged off its spot sits at the offset stored on the opening.
pub fn opening_labels(cx: &EditorContext) -> Vec<OpeningLabel> {
    let floor = cx.floor();
    if floor.openings.is_empty() {
        return Vec::new();
    }
    let h = schedule_view::label_style(&cx.project).map_or(4.5, |s| s.height_in);
    let marks = marks(cx);
    let mut out = Vec::new();
    for o in &floor.openings {
        if !labels_visible(cx, o.kind) {
            continue;
        }
        let Some(wall) = floor.wall(o.wall_id) else {
            continue;
        };
        let Some((text, mark, base)) = label_base(cx, o, wall, &marks, h) else {
            continue;
        };
        let at = apply_label_offset(wall, base, o.extras.spec.label_offset);
        let is_mark = mark.is_some_and(|m| m == text);
        out.push(OpeningLabel {
            opening: o.id,
            kind: o.kind,
            text,
            is_mark,
            at,
        });
    }
    out
}

/// Drags the label of opening `id` so its center is at `to`: stores the
/// offset from the unmoved spot on the opening (DW-63). `false` when the
/// opening has no label to move.
pub fn drag_label(cx: &mut EditorContext, id: Id, to: Point) -> bool {
    let floor = cx.floor();
    let Some(o) = floor.openings.iter().find(|o| o.id == id) else {
        return false;
    };
    let Some(wall) = floor.wall(o.wall_id) else {
        return false;
    };
    let h = schedule_view::label_style(&cx.project).map_or(4.5, |s| s.height_in);
    let marks = marks(cx);
    let Some((_, _, base)) = label_base(cx, o, wall, &marks, h) else {
        return false;
    };
    let d = to.sub(base);
    let s = wall.locate(base).0;
    let offset = (d.dot(wall.tangent_along(s)), d.dot(wall.normal_along(s)));
    let fl = cx.floor;
    match cx.project.floors[fl]
        .openings
        .iter_mut()
        .find(|o| o.id == id)
    {
        Some(o) => {
            o.extras.spec.label_offset = offset;
            true
        }
        None => false,
    }
}

/// Casing as small rectangles on the wall faces, once around a mulled unit
/// (DW-52, DW-79), for the openings that ask for it.
fn draw_casing(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let floor = cx.floor();
    let pal = &cx.palette;
    for o in &floor.openings {
        if !o.extras.spec.casing_in_plan {
            continue;
        }
        let layer = match o.kind {
            OpeningKind::Door => "Doors",
            OpeningKind::Window => "Windows",
        };
        if !cx.layers().is_visible(layer) {
            continue;
        }
        let Some(wall) = floor.wall(o.wall_id) else {
            continue;
        };
        // A window over a door has no casing of its own: the unit's is drawn
        // once, by the member below.
        let Some(unit) = cx.project.casing_unit(cx.floor, o.id) else {
            continue;
        };
        let ext = exterior_sign(wall, &cx.rooms);
        for part in plan_core::opening_symbol::casing_parts(wall, o, unit, ext) {
            let pts = screen(cam, &part.points);
            painter.add(Shape::closed_line(
                pts,
                Stroke::new(0.8_f32, pal.opening_line),
            ));
        }
        // Openings whose casings touch share one casing as wide as the gap
        // between them (manual p. 608).
        for quad in shared_casing_quads(floor, wall, o, ext) {
            painter.add(Shape::closed_line(
                screen(cam, &quad),
                Stroke::new(0.8_f32, pal.opening_line),
            ));
        }
    }
}

/// The rectangles of the casing `o` shares with the opening beside it on its
/// end side, on both faces of `wall`.
pub fn shared_casing_quads(
    floor: &Floor,
    wall: &Wall,
    o: &Opening,
    exterior: f64,
) -> Vec<[Point; 4]> {
    let spec = &o.extras.spec;
    let c = o.casing.unwrap_or_default();
    let half = wall.thickness * 0.5;
    let mut out = Vec::new();
    let next = floor
        .openings_on(wall.id)
        .filter(|n| n.id != o.id && n.start_offset() >= o.end_offset() - 1e-9)
        .filter(|n| plan_core::openings::mull::auto_mulled(o, n))
        .min_by(|a, b| a.start_offset().total_cmp(&b.start_offset()));
    let Some(n) = next else {
        return out;
    };
    let gap = n.start_offset() - o.end_offset();
    if gap <= 0.01 {
        return out;
    }
    for side in [1.0, -1.0] {
        let is_exterior = side == exterior;
        if (is_exterior && !spec.casing_exterior) || (!is_exterior && !spec.casing_interior) {
            continue;
        }
        let (t0, t1) = (side * half, side * (half + c.depth));
        out.extend(wall.band_quads(o.end_offset(), n.start_offset(), t0.min(t1), t0.max(t1)));
    }
    out
}

/// The name of the layer that shows the headers over windows and doors.
pub const HEADER_LAYER: &str = "Opening Header Lines";

/// With the "Opening Header Lines" layer on, a dashed line across each window
/// and door stands for its header (manual p. 612). The lines are not framing
/// and cannot be selected.
fn draw_header_lines(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    if !cx.layers().get(HEADER_LAYER).is_some_and(|l| l.display) {
        return;
    }
    let floor = cx.floor();
    let pal = &cx.palette;
    for o in &floor.openings {
        let Some(wall) = floor.wall(o.wall_id) else {
            continue;
        };
        let pts = [
            wall.point_along(o.start_offset()),
            wall.point_along(o.end_offset()),
        ];
        let screen: Vec<Pos2> = pts.iter().map(|p| cam.world_to_screen(*p)).collect();
        painter.extend(Shape::dashed_line(
            &screen,
            Stroke::new(0.8_f32, pal.opening_line),
            DASH_PX.0,
            DASH_PX.1,
        ));
    }
}

/// The Caution symbol over four or more openings in one place, and the width
/// and radius dimensions of bay, box and bow windows.
fn draw_cautions(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let floor = cx.floor();
    for c in cx.project.stacked_clusters(cx.floor) {
        let Some(wall) = floor.wall(c.wall_id) else {
            continue;
        };
        let at = cam.world_to_screen(wall.point_along(c.at));
        let r = 9.0_f32;
        let tri = vec![
            at + Vec2::new(0.0, -r),
            at + Vec2::new(r, r * 0.8),
            at + Vec2::new(-r, r * 0.8),
        ];
        painter.add(Shape::convex_polygon(
            tri,
            Color32::from_rgb(0xF2, 0xC1, 0x2E),
            Stroke::new(1.0_f32, Color32::BLACK),
        ));
        painter.text(
            at + Vec2::new(0.0, 2.0),
            egui::Align2::CENTER_CENTER,
            "!",
            FontId::proportional(11.0),
            Color32::BLACK,
        );
    }
}

/// Bay, box and bow window dimensions in plan (Options panel: Display Standard
/// Dimension, Display Dimensions to Center), on the Dimensions layer.
fn draw_bay_dimensions(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    use plan_core::openings::bay::{bay_dimensions, BayDimKind};
    if !cx.layers().is_visible("Dimensions") {
        return;
    }
    let floor = cx.floor();
    let pal = &cx.palette;
    for o in floor.openings.iter().filter(|o| o.style.projects()) {
        let Some(wall) = floor.wall(o.wall_id) else {
            continue;
        };
        let ext = exterior_sign(wall, &cx.rooms);
        let sign = if o.swing_flipped { -ext } else { ext };
        let half = wall.thickness * 0.5;
        let world = |p: (f64, f64)| {
            let s = o.start_offset() + p.0;
            wall.point_along(s)
                .add(wall.normal_along(s).scale(sign * (half + p.1)))
        };
        for d in bay_dimensions(o.style, o.width, &o.extras.spec.bay) {
            let (mut a, mut b) = (world(d.a), world(d.b));
            // Width runs a little outside the front; depth beside the unit.
            let off = match d.kind {
                BayDimKind::Width => wall.normal_along(o.center_offset).scale(sign * 6.0),
                BayDimKind::Depth => wall.tangent_along(o.center_offset).scale(-4.0),
                BayDimKind::Radius => Point::ZERO,
            };
            a = a.add(off);
            b = b.add(off);
            let (sa, sb) = (cam.world_to_screen(a), cam.world_to_screen(b));
            painter.line_segment([sa, sb], Stroke::new(0.8_f32, pal.opening_line));
            let text = match d.kind {
                BayDimKind::Radius => format!("R {}", plan_core::units::fmt_ft_in(d.value)),
                _ => plan_core::units::fmt_ft_in(d.value),
            };
            painter.text(
                Pos2::new((sa.x + sb.x) * 0.5, (sa.y + sb.y) * 0.5 - 6.0),
                egui::Align2::CENTER_BOTTOM,
                text,
                FontId::proportional(10.0),
                pal.text,
            );
        }
    }
}

/// Draws the opening labels over the plan (and the casing rectangles, which
/// belong to the same pass).
pub fn draw_opening_labels(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    draw_casing(cx, painter, cam);
    draw_header_lines(cx, painter, cam);
    draw_bay_dimensions(cx, painter, cam);
    draw_cautions(cx, painter, cam);
    let style = schedule_view::label_style(&cx.project);
    let h = style.map_or(4.5, |s| s.height_in);
    let font_px = (h as f32 * cam.px_per_in as f32).clamp(1.0, 300.0);
    if font_px < MIN_LABEL_PX {
        return;
    }
    let pal = &cx.palette;
    let ink = match style {
        Some(s) if s.color != [0, 0, 0] => Color32::from_rgb(s.color[0], s.color[1], s.color[2]),
        _ => pal.text,
    };
    for l in opening_labels(cx) {
        let at = cam.world_to_screen(l.at);
        if !cam.rect.expand(60.0).contains(at) {
            continue;
        }
        if l.is_mark {
            // The mark in its bubble, like the schedule's own callouts.
            let kind = match l.kind {
                OpeningKind::Door => plan_core::schedules::ScheduleKind::Door,
                OpeningKind::Window => plan_core::schedules::ScheduleKind::Window,
            };
            let callout = Callout {
                kind,
                floor: cx.floor,
                object: l.opening,
                text: l.text,
                at: l.at,
            };
            schedule_view::draw_label(painter, cam, &callout, h, ink, pal.background);
        } else {
            let galley = painter.layout_no_wrap(l.text.clone(), FontId::proportional(font_px), ink);
            let r = Rect::from_center_size(at, galley.size() + Vec2::new(4.0, 2.0));
            painter.rect_filled(r, 2.0, pal.background.gamma_multiply(0.85));
            painter.galley(r.center() - galley.size() * 0.5, galley, ink);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::schedules::ScheduleKind;
    use plan_core::{LabelMode, SizeFormat, SizeStyle, WallKind};

    fn cx_with_wall() -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            109.125,
            WallKind::Exterior,
        );
        (cx, w)
    }

    fn labels_of(cx: &mut EditorContext) -> Vec<OpeningLabel> {
        cx.mark_dirty();
        cx.refresh();
        opening_labels(cx)
    }

    #[test]
    fn labels_show_the_size_without_a_schedule() {
        let (mut cx, w) = cx_with_wall();
        let d = cx
            .project
            .add_opening(0, w, 60.0, OpeningKind::Door)
            .unwrap();
        let win = cx
            .project
            .add_opening(0, w, 150.0, OpeningKind::Window)
            .unwrap();
        let l = labels_of(&mut cx);
        let text = |id| l.iter().find(|x| x.opening == id).unwrap().text.clone();
        assert_eq!(text(d), "3068");
        assert_eq!(text(win), "3050");
        assert!(l.iter().all(|x| !x.is_mark));
    }

    #[test]
    fn labels_follow_the_default_settings_formats() {
        let (mut cx, w) = cx_with_wall();
        let d = cx
            .project
            .add_opening(0, w, 60.0, OpeningKind::Door)
            .unwrap();
        cx.defaults.opening_labels.door.size_style = SizeStyle::Architectural;
        let l = labels_of(&mut cx);
        assert_eq!(l[0].text, "3'-0\" x 6'-8\"");
        cx.defaults.opening_labels.door.size_format = SizeFormat::WidthOnly;
        let l = labels_of(&mut cx);
        assert_eq!(l[0].text, "3'-0\"");
        cx.defaults.opening_labels.door.mode = LabelMode::Suppress;
        assert!(labels_of(&mut cx).is_empty());
        cx.defaults.opening_labels.door.mode = LabelMode::Automatic;
        // The opening's own text beats the defaults.
        cx.project.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.id == d)
            .unwrap()
            .label_override = Some("EXIT".into());
        assert_eq!(labels_of(&mut cx)[0].text, "EXIT");
        // A hidden layer hides the label.
        cx.project.layers.set_display("Doors", false);
        assert!(labels_of(&mut cx).is_empty());
    }

    #[test]
    fn a_schedule_turns_the_size_into_the_mark() {
        let (mut cx, w) = cx_with_wall();
        let d1 = cx
            .project
            .add_opening(0, w, 200.0, OpeningKind::Door)
            .unwrap();
        let d2 = cx
            .project
            .add_opening(0, w, 60.0, OpeningKind::Door)
            .unwrap();
        let win = cx
            .project
            .add_opening(0, w, 130.0, OpeningKind::Window)
            .unwrap();
        let sid = schedule_view::add(&mut cx, ScheduleKind::Door, Point::new(0.0, -80.0));
        let mut def = schedule_view::find(&cx, sid).unwrap();
        def.show_labels = true;
        assert!(schedule_view::replace(&mut cx, 0, def));
        let l = labels_of(&mut cx);
        let get = |id| l.iter().find(|x| x.opening == id).unwrap().clone();
        // Alike doors are numbered in the order they were placed (p. 715).
        assert_eq!((get(d1).text.as_str(), get(d1).is_mark), ("D01", true));
        assert_eq!((get(d2).text.as_str(), get(d2).is_mark), ("D02", true));
        // The window has no schedule: still its size.
        assert_eq!((get(win).text.as_str(), get(win).is_mark), ("3050", false));
        // A new door goes to the bottom of the schedule, wherever it is
        // (manual p. 715).
        let d3 = cx
            .project
            .add_opening(0, w, 10.0, OpeningKind::Door)
            .unwrap();
        let l = labels_of(&mut cx);
        let marks: Vec<String> = [d3, d1, d2]
            .iter()
            .map(|id| l.iter().find(|x| x.opening == *id).unwrap().text.clone())
            .collect();
        assert_eq!(marks, ["D03", "D01", "D02"]);
        // Removing it releases the numbers again.
        cx.project.floors[0].openings.retain(|o| o.id != d3);
        let l = labels_of(&mut cx);
        assert_eq!(l.iter().find(|x| x.opening == d1).unwrap().text, "D01");
        assert_eq!(l.iter().find(|x| x.opening == d2).unwrap().text, "D02");
    }

    #[test]
    fn placement_puts_the_label_on_or_beside_the_wall() {
        let (mut cx, w) = cx_with_wall();
        let d = cx
            .project
            .add_opening(0, w, 120.0, OpeningKind::Door)
            .unwrap();
        let wall = cx.floor().wall(w).unwrap().clone();
        let o = cx
            .floor()
            .openings
            .iter()
            .find(|o| o.id == d)
            .unwrap()
            .clone();
        let ext = exterior_sign(&wall, &[]);
        assert_eq!(ext, 1.0);
        let center = label_anchor(&wall, &o, LabelPlacement::Center, ext, 20.0, 4.5);
        assert!((center.y).abs() < 1e-9 && (center.x - 120.0).abs() < 1e-9);
        let outside = label_anchor(&wall, &o, LabelPlacement::Exterior, ext, 20.0, 4.5);
        let inside = label_anchor(&wall, &o, LabelPlacement::Interior, ext, 20.0, 4.5);
        // Clear of the wall (3" half thickness) on opposite sides.
        assert!(outside.y > 3.0 && inside.y < -3.0, "{outside:?} {inside:?}");
        assert!((outside.x - 120.0).abs() < 1e-9);
        // A vertical wall pushes the label out by the text's half width.
        let v = Wall::new(
            Point::ZERO,
            Point::new(0.0, 240.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let vo = Opening::new(1, 120.0, OpeningKind::Door, 36.0, 80.0, 0.0);
        let side = label_anchor(&v, &vo, LabelPlacement::Exterior, 1.0, 20.0, 4.5);
        assert!((side.x.abs() - (3.0 + 1.5 + 10.0)).abs() < 1e-9, "{side:?}");
    }

    #[test]
    fn labels_have_their_own_layers_that_hide_them_alone() {
        let (mut cx, w) = cx_with_wall();
        // A plan from before the label layers gets them with its first opening
        // (a new plan has them already).
        cx.project
            .layers
            .layers
            .retain(|l| !l.name.ends_with(", Labels"));
        cx.project.ensure_opening_label_layers();
        let d = cx
            .project
            .add_opening(0, w, 60.0, OpeningKind::Door)
            .unwrap();
        let win = cx
            .project
            .add_opening(0, w, 150.0, OpeningKind::Window)
            .unwrap();
        // The default layer set carries them, beside the object layers.
        let names: Vec<&str> = cx
            .project
            .layers
            .layers
            .iter()
            .map(|l| l.name.as_str())
            .collect();
        let i = names.iter().position(|n| *n == "Doors").unwrap();
        assert_eq!(names[i + 1], "Doors, Labels");
        assert!(names.contains(&"Windows, Labels"));
        assert_eq!(labels_of(&mut cx).len(), 2);
        // Hiding "Doors, Labels" hides the door label only; the door stays.
        cx.project.layers.set_display("Doors, Labels", false);
        let l = labels_of(&mut cx);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].opening, win);
        assert!(cx.layers().is_visible("Doors"));
        cx.project.layers.set_display("Doors, Labels", true);
        cx.project.layers.set_display("Windows, Labels", false);
        let l = labels_of(&mut cx);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].opening, d);
        // Hiding the object layer hides its label too.
        cx.project.layers.set_display("Windows, Labels", true);
        cx.project.layers.set_display("Doors", false);
        assert_eq!(labels_of(&mut cx).len(), 1);
    }

    #[test]
    fn a_dragged_label_keeps_an_offset_on_the_opening_and_the_mark_follows() {
        let (mut cx, w) = cx_with_wall();
        let d = cx
            .project
            .add_opening(0, w, 60.0, OpeningKind::Door)
            .unwrap();
        let home = labels_of(&mut cx)[0].at;
        // Drag it 20" along the wall and 10" farther into the room.
        let to = Point::new(home.x + 20.0, home.y - 10.0);
        assert!(drag_label(&mut cx, d, to));
        let o = cx
            .floor()
            .openings
            .iter()
            .find(|o| o.id == d)
            .unwrap()
            .clone();
        let n = cx.floor().wall(w).unwrap().normal();
        assert!((o.extras.spec.label_offset.0 - 20.0).abs() < 1e-9);
        assert!((o.extras.spec.label_offset.1 - -10.0 * n.y).abs() < 1e-9);
        let moved = labels_of(&mut cx)[0].at;
        assert!(moved.dist(to) < 1e-9, "{moved:?} vs {to:?}");
        // It follows the opening when the wall's opening slides.
        let slid = cx.project.slide_opening(0, d, 100.0);
        assert!(slid);
        let after = labels_of(&mut cx)[0].at;
        assert!((after.x - (to.x + 40.0)).abs() < 1e-9 && (after.y - to.y).abs() < 1e-9);
        // The offset survives a save and a load.
        let json = serde_json::to_string(&cx.project).unwrap();
        let back: plan_core::Project = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.floors[0].openings[0].extras.spec.label_offset,
            o.extras.spec.label_offset
        );
        // A suppressed label cannot be dragged.
        cx.defaults.opening_labels.door.mode = LabelMode::Suppress;
        cx.mark_dirty();
        cx.refresh();
        assert!(!drag_label(&mut cx, d, home));
    }

    #[test]
    fn the_schedule_mark_bubble_follows_the_dragged_label() {
        let (mut cx, w) = cx_with_wall();
        let d = cx
            .project
            .add_opening(0, w, 60.0, OpeningKind::Door)
            .unwrap();
        let sid = schedule_view::add(&mut cx, ScheduleKind::Door, Point::new(0.0, -80.0));
        let mut def = schedule_view::find(&cx, sid).unwrap();
        def.show_labels = true;
        assert!(schedule_view::replace(&mut cx, 0, def));
        let l = labels_of(&mut cx);
        assert!(l[0].is_mark);
        let to = Point::new(l[0].at.x - 30.0, l[0].at.y);
        drag_label(&mut cx, d, to);
        let l = labels_of(&mut cx);
        assert!(l[0].is_mark && l[0].at.dist(to) < 1e-9);
    }

    /// A 20' chord bowed 60" (radius 150") with a 36" door at arc length 100.
    fn curved_cx() -> (EditorContext, Id, Id) {
        let (mut cx, w) = cx_with_wall();
        cx.project.floors[0].walls[0].curve = Some(plan_core::walls::WallCurve { bulge: 60.0 });
        let d = cx
            .project
            .add_opening(0, w, 100.0, OpeningKind::Door)
            .unwrap();
        (cx, w, d)
    }

    #[test]
    fn the_hit_test_and_the_labels_follow_the_arc() {
        let (mut cx, w, d) = curved_cx();
        let wall = cx.floor().wall(w).unwrap().clone();
        // The middle of the door on the arc hits it; the chord point at the
        // same distance along does not (it is far off the wall).
        let on_arc = wall.point_along(100.0);
        assert_eq!(
            super::super::selection::hit_opening(cx.floor(), on_arc, 1.0),
            Some(d)
        );
        let on_chord = wall.point_at(100.0);
        assert_eq!(
            super::super::selection::hit_opening(cx.floor(), on_chord, 1.0),
            None
        );
        // Just inside the jamb on the arc hits, just past it does not.
        assert_eq!(
            super::super::selection::hit_opening(cx.floor(), wall.point_along(83.0), 0.5),
            Some(d)
        );
        assert_eq!(
            super::super::selection::hit_opening(cx.floor(), wall.point_along(80.0), 0.5),
            None
        );
        // The label sits on the arc at the door's center, and a dragged offset
        // is measured along the tangent there.
        let l = labels_of(&mut cx);
        let at = l.iter().find(|x| x.opening == d).unwrap().at;
        let (s, lateral) = wall.locate(at);
        assert!((s - 100.0).abs() < 1e-6, "{s}");
        assert!(lateral.abs() > wall.thickness * 0.5, "{lateral}");
        let moved = apply_label_offset(&wall, at, (10.0, 0.0));
        assert!((wall.locate(moved).0 - 110.0).abs() < 0.6);
    }

    #[test]
    fn the_width_dimension_reads_the_arc() {
        let (cx, w, d) = curved_cx();
        let wall = cx.floor().wall(w).unwrap();
        let o = cx.floor().openings.iter().find(|o| o.id == d).unwrap();
        let dims = super::super::tempdim::opening_temp_dims(
            cx.floor(),
            wall,
            o,
            super::super::selection::ObjectRef::Opening(d),
            &super::super::tempdim::TempLocate::default(),
        );
        let width = dims
            .iter()
            .find(|x| x.kind == super::super::tempdim::TempDimKind::OpeningWidth)
            .unwrap();
        // 36" of arc, between the jambs on the centerline arc (a shorter chord).
        assert_eq!(width.value, 36.0);
        assert!(width.a.dist(wall.point_along(82.0)) < 1e-9);
        assert!(width.b.dist(wall.point_along(118.0)) < 1e-9);
        assert!(width.a.dist(width.b) < 36.0);
        // The distance to the wall start reads the arc too, not the chord.
        let to_start = dims
            .iter()
            .find(|x| x.kind == super::super::tempdim::TempDimKind::OpeningToStart)
            .unwrap();
        assert_eq!(to_start.value, 82.0);
        let to_end = dims
            .iter()
            .find(|x| x.kind == super::super::tempdim::TempDimKind::OpeningToEnd)
            .unwrap();
        assert!((to_end.value - (wall.path_length() - 118.0)).abs() < 1e-9);
    }
}
