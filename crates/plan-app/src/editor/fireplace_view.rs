//! Fireplaces and chimneys in the editor (CB-87), with the decks (CB-86) and
//! split-level floors (R-86) that share its plan drawing in [`deck`].
//!
//! A fireplace is a placed symbol (`catalog_id` `plan-studio.fireplace` or
//! `plan-studio.chimney`) plus a [`Fireplace`] record in `Floor::fireplaces`
//! with the same id (see `plan_core::fireplace`). The Select tool therefore
//! moves, rotates, sizes, copies and deletes it as it does any symbol; this
//! module places, specifies and draws it.
//!
//! * [`placement_at`] finds where a fireplace goes under the pointer: flush
//!   against the wall face it is beside, built into the wall (the wall is cut
//!   in 3D), or free.
//! * [`place`] and [`apply`] are one undo step each.
//! * [`draw`] adds what the symbol outline does not show: the firebox, the
//!   hearth, the mantel, the chimney's X and the chimney chase on the floors
//!   above.
//! * [`chimney_holes`] are the outlines the roof cuts for chimneys.
//! * [`edit_actions`] and [`run_command`] are the Edit toolbar commands.

pub mod deck;

use super::actions::{EditAction, EditActionKind};
use super::selection::ObjectRef;
use super::{Camera, EditorContext, EditorRequest};
use crate::tools::ToolId;
use eframe::egui::{self, Color32, Pos2, Shape, Stroke};
use plan_core::deck::offset_polygon;
use plan_core::fireplace::{
    body_poly, chases_on_floor, chimney_poly, firebox_poly, hearth_poly, is_fireplace_symbol,
    mantel_poly, plan_dimensions, rough_poly, Fireplace, FireplaceKind, Frame, FIREPLACE_LAYER,
};
use plan_core::geometry::{dist_to_segment, point_in_polygon, Point};
use plan_core::walls::WallClass;
use plan_core::{Floor, Id, Layer, PlacedSymbol, Project, Wall};

/// Edit toolbar command ids.
pub mod cmd {
    pub const OPEN_SPEC: &str = "fireplace.open_spec";
    pub const BUILD_DECK: &str = "deck.build_framing";
    pub const CLEAR_DECK: &str = "deck.clear_framing";
    pub const ADD_STEPS: &str = "split.add_steps";
}

/// How near a wall the pointer must be to snap a fireplace to it, beyond the
/// wall's half thickness, inches.
const WALL_REACH: f64 = 30.0;
/// Clearance the roof leaves around a chimney, inches.
pub const ROOF_CLEARANCE: f64 = 2.0;

// ----- finding the specification of a symbol -----

/// The symbol and specification of the fireplace `id` on `floor`.
pub fn load(floor: &Floor, id: Id) -> Option<(PlacedSymbol, Fireplace)> {
    let sym = floor.symbol(id).filter(|s| is_fireplace_symbol(s))?;
    Some((sym.clone(), floor.fireplace_of(sym)))
}

/// Is the placed object `id` of `floor` a fireplace or chimney?
pub fn is_fireplace(floor: &Floor, id: Id) -> bool {
    floor.symbol(id).is_some_and(is_fireplace_symbol)
}

/// The fireplace the selection is exactly one of, if any.
pub fn selected(cx: &EditorContext) -> Option<Id> {
    match cx.selection.single() {
        Some(ObjectRef::Symbol(id)) if is_fireplace(cx.floor(), id) => Some(id),
        _ => None,
    }
}

/// Makes sure the Fireplaces layer exists.
pub fn ensure_layer(project: &mut Project) {
    if project.layers.get(FIREPLACE_LAYER).is_none() {
        project
            .layers
            .add(Layer::new(FIREPLACE_LAYER, [150, 70, 40], 25));
    }
}

/// The label text the symbol shows: the record's label, a space when the
/// label is hidden (a symbol with no label shows its catalog id).
fn symbol_label(fp: &Fireplace) -> String {
    if fp.label.show {
        fp.label_text()
    } else {
        " ".to_string()
    }
}

// ----- pending "open the specification" (the tool hosts the dialog) -----

thread_local! {
    /// The editor runs on the UI thread, so the request is per thread.
    static PENDING_OPEN: std::cell::Cell<Option<Id>> = const { std::cell::Cell::new(None) };
}

/// Asks the Fireplace tool to open the specification of fireplace `id`.
pub fn request_open(id: Id) {
    PENDING_OPEN.with(|p| p.set(Some(id)));
}

/// Takes the pending request.
pub fn take_open() -> Option<Id> {
    PENDING_OPEN.with(std::cell::Cell::take)
}

// ----- placing -----

/// Where a new fireplace goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Middle of the back of the body.
    pub position: Point,
    /// Degrees; the front faces +y at 0.
    pub angle: f64,
    /// The wall the fireplace stands against or in.
    pub wall: Option<Id>,
    pub in_wall: bool,
}

/// Walls a fireplace can stand against: solid walls, not railings, room
/// dividers or invisible ones.
fn host_wall(w: &Wall) -> bool {
    !(w.flags.invisible
        || w.flags.room_divider
        || w.flags.railing
        || w.is_curved()
        || matches!(
            w.class,
            WallClass::RoomDivider
                | WallClass::Railing
                | WallClass::DeckRailing
                | WallClass::DeckEdge
                | WallClass::Fencing { .. }
        ))
        && w.length() > 12.0
}

/// The placement for a fireplace of `kind` under the pointer at `p`.
///
/// Near a wall (within the wall's half thickness plus a reach) the fireplace
/// turns its back to the wall: beside the wall, its back flush with the wall
/// face on the side it faces; or, for `in_wall`, with its back flush with the
/// outside of the wall and the body reaching through it (manual p. 757).
/// It faces the side of the pointer, except that on an exterior wall it
/// always faces the interior (DECISIONS 202 as corrected). Elsewhere it
/// stands free, centered on `p`, facing `free_angle`.
pub fn placement_at(
    floor: &Floor,
    p: Point,
    kind: FireplaceKind,
    in_wall: bool,
    free_angle: f64,
) -> Placement {
    let (w, d) = Fireplace::default_size(kind);
    let nearest = floor
        .walls
        .iter()
        .filter(|wl| host_wall(wl))
        .map(|wl| {
            (
                dist_to_segment(p, wl.start, wl.end) - wl.thickness * 0.5,
                wl,
            )
        })
        .filter(|(gap, _)| *gap <= WALL_REACH)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, wall)) = nearest else {
        let a = free_angle.to_radians();
        let v = Point::new(-a.sin(), a.cos());
        return Placement {
            position: p - v * (d * 0.5),
            angle: free_angle,
            wall: None,
            in_wall: false,
        };
    };
    let dir = wall.direction();
    // Slide along the wall, keeping the body on it.
    let along = (p - wall.start)
        .dot(dir)
        .clamp(w * 0.5, (wall.length() - w * 0.5).max(w * 0.5));
    let foot = wall.start + dir * along;
    let mut side = if (p - foot).dot(wall.normal()) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    // On an exterior wall the fireplace always faces the room, whichever
    // side the pointer is on (when the plan has a room to tell the sides by).
    if wall.kind == plan_core::WallKind::Exterior {
        if let Some(interior) = interior_side(floor, wall) {
            side = interior;
        }
    }
    let n = wall.normal() * side;
    // The front faces the pointer's side: v = n, so u = v turned back 90.
    let u = Point::new(n.y, -n.x);
    let angle = u.y.atan2(u.x).to_degrees();
    let face = foot + n * (wall.thickness * 0.5);
    // Built in: the back is flush with the far face, the front stands out
    // into the room by what the body is deeper than the wall is thick.
    let position = if in_wall {
        foot - n * (wall.thickness * 0.5)
    } else {
        face
    };
    Placement {
        position,
        angle,
        wall: Some(wall.id),
        in_wall,
    }
}

/// Which side of `wall` is the room: `1.0` for the +normal side, `-1.0` for
/// the other; `None` when no room lies beside the wall to say.
pub fn interior_side(floor: &Floor, wall: &Wall) -> Option<f64> {
    let rooms = plan_core::rooms::detect_rooms(&floor.walls, 0.5);
    let along = wall.path_length() * 0.5;
    let mid = wall.point_along(along);
    let reach = wall.thickness * 0.5 + 1.5;
    let n = wall.normal_along(along);
    let inside = |sign: f64| {
        rooms
            .iter()
            .any(|r| point_in_polygon(mid + n * (sign * reach), &r.polygon))
    };
    match (inside(1.0), inside(-1.0)) {
        (true, false) => Some(1.0),
        (false, true) => Some(-1.0),
        _ => None,
    }
}

/// The Depth handle of a fireplace built into a wall (manual p. 757): drag it
/// toward the outside of the wall and the fireplace slides that way, until
/// the front of the fireplace is flush with the inside edge of the wall. The
/// symbol after dragging from `start` to `to`; `None` for a fireplace that is
/// not built into a wall, which resizes like any symbol.
pub fn slide_in_wall(
    floor: &Floor,
    orig: &PlacedSymbol,
    start: Point,
    to: Point,
) -> Option<PlacedSymbol> {
    let in_wall = floor
        .fireplace_symbols()
        .iter()
        .any(|(s, f)| s.id == orig.id && f.in_wall);
    if !in_wall {
        return None;
    }
    let frame = Frame::of(orig);
    let middle = frame.at(0.0, orig.depth * 0.5);
    let wall = floor
        .walls
        .iter()
        .filter(|w| host_wall(w))
        .map(|w| (dist_to_segment(middle, w.start, w.end), w))
        .filter(|(d, _)| *d <= orig.depth * 0.5 + WALL_REACH)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, w)| w)?;
    let dir = wall.direction();
    let foot = wall.start + dir * (middle - wall.start).dot(dir).clamp(0.0, wall.length());
    // The inside edge of the wall, along the fireplace's own depth axis.
    let inside = (foot - frame.origin).dot(frame.v) + wall.thickness * 0.5;
    let slid = (to - start).dot(frame.v).clamp(inside - orig.depth, 0.0);
    let mut out = orig.clone();
    out.position = frame.origin + frame.v * slid;
    Some(out)
}

/// Places a fireplace of `kind` as one undo step and selects it.
pub fn place(cx: &mut EditorContext, kind: FireplaceKind, pl: Placement) -> Id {
    let label = if kind == FireplaceKind::ChimneyOnly {
        "Place Chimney"
    } else {
        "Place Fireplace"
    };
    cx.begin_change(label);
    ensure_layer(&mut cx.project);
    let fl = cx.floor;
    let id = cx.project.add_fireplace(fl, kind, pl.position, pl.angle);
    {
        let floor = &mut cx.project.floors[fl];
        let mut fp = floor
            .fireplaces
            .iter()
            .find(|f| f.id == id)
            .cloned()
            .unwrap_or_else(|| Fireplace::new(id, kind));
        fp.in_wall = pl.in_wall;
        let text = symbol_label(&fp);
        if let Some(s) = floor.symbols.iter_mut().find(|s| s.id == id) {
            s.label = text;
        }
        floor.set_fireplace(fp);
    }
    cx.selection.set(ObjectRef::Symbol(id));
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!(
        "Placed {}",
        match (kind, pl.in_wall) {
            (FireplaceKind::ChimneyOnly, _) => "a chimney",
            (_, true) => "a fireplace built into the wall",
            _ => "a fireplace",
        }
    );
    id
}

/// Writes the edited specification and symbol of one fireplace (one undo
/// step). Returns false when the symbol is gone.
pub fn apply(cx: &mut EditorContext, sym: &PlacedSymbol, fp: &Fireplace) -> bool {
    if !is_fireplace(cx.floor(), sym.id) {
        return false;
    }
    cx.begin_change("Fireplace Specification");
    let mut fp = fp.clone();
    fp.id = sym.id;
    fp.fit_to(sym.width, sym.depth);
    let mut sym = sym.clone();
    sym.label = symbol_label(&fp);
    ensure_layer(&mut cx.project);
    let floor = cx.floor_mut();
    if let Some(slot) = floor.symbols.iter_mut().find(|s| s.id == sym.id) {
        *slot = sym;
    }
    floor.set_fireplace(fp);
    cx.mark_dirty();
    cx.refresh();
    true
}

/// Drops the records of fireplaces whose symbol was deleted; call after a
/// delete. Returns how many went.
pub fn drop_orphans(cx: &mut EditorContext) -> usize {
    if cx.floor().fireplaces.is_empty() {
        return 0;
    }
    let alive = |cx: &EditorContext| {
        cx.floor()
            .fireplaces
            .iter()
            .filter(|f| is_fireplace(cx.floor(), f.id))
            .count()
    };
    if alive(cx) == cx.floor().fireplaces.len() {
        return 0;
    }
    // The delete already took its undo step; the records go with it.
    cx.floor_mut().prune_fireplaces()
}

// ----- picking -----

/// The fireplace under `p`: its body, hearth, mantel or chimney.
pub fn hit(floor: &Floor, p: Point, tol: f64) -> Option<Id> {
    floor.fireplace_symbols().into_iter().find_map(|(sym, fp)| {
        let polys = [
            body_poly(sym),
            hearth_poly(&fp, sym),
            mantel_poly(&fp, sym),
            chimney_poly(&fp, sym),
        ];
        polys
            .iter()
            .any(|poly| poly.len() >= 3 && (point_in_polygon(p, poly) || near(poly, p, tol)))
            .then_some(sym.id)
    })
}

fn near(poly: &[Point], p: Point, tol: f64) -> bool {
    let n = poly.len();
    (0..n).any(|i| dist_to_segment(p, poly[i], poly[(i + 1) % n]) <= tol)
}

// ----- the roof -----

/// The outlines the roof planes cut for the chimneys of the project:
/// `(floor index, outline)`, each grown by [`ROOF_CLEARANCE`].
pub fn chimney_holes(project: &Project) -> Vec<(usize, Vec<Point>)> {
    let mut out = Vec::new();
    for (i, floor) in project.floors.iter().enumerate() {
        for (sym, fp) in floor.fireplace_symbols() {
            if !fp.chimney.enabled {
                continue;
            }
            let poly = chimney_poly(&fp, sym);
            let grown = offset_polygon(&poly, -ROOF_CLEARANCE).unwrap_or(poly);
            out.push((i, grown));
        }
    }
    out
}

// ----- the Edit toolbar -----

/// The commands of the selection: Fireplace Specification for a fireplace.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    if selected(cx).is_some() {
        v.push(EditAction::new(EditActionKind::Custom {
            id: cmd::OPEN_SPEC,
            label: "Fireplace Specification",
            icon: "",
        }));
    }
    v.extend(deck::edit_actions(cx));
    v
}

/// Runs one of the commands in [`cmd`]. Returns whether `id` was one.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        cmd::OPEN_SPEC => {
            if let Some(fid) = selected(cx) {
                request_open(fid);
                cx.requests.push(EditorRequest::SetTool(ToolId::Fireplace));
            }
            true
        }
        cmd::BUILD_DECK => {
            deck::build_framing(cx);
            true
        }
        cmd::CLEAR_DECK => {
            deck::clear_framing(cx);
            true
        }
        cmd::ADD_STEPS => {
            deck::add_steps(cx);
            true
        }
        _ => false,
    }
}

// ----- drawing -----

fn sc(cam: &Camera, p: Point) -> Pos2 {
    cam.world_to_screen(p)
}

fn closed(cam: &Camera, poly: &[Point]) -> Vec<Pos2> {
    poly.iter().map(|p| sc(cam, *p)).collect()
}

fn dashed(painter: &egui::Painter, cam: &Camera, poly: &[Point], stroke: Stroke) {
    let mut pts = closed(cam, poly);
    if let Some(first) = pts.first().copied() {
        pts.push(first);
    }
    painter.extend(Shape::dashed_line(&pts, stroke, 6.0, 4.0));
}

/// Brick hatch lines of a body: diagonals every `spacing` inches across the
/// rectangle `x` by `y` of `frame`, as plan segments.
pub fn hatch(frame: &Frame, w: f64, d: f64, spacing: f64) -> Vec<(Point, Point)> {
    let mut out = Vec::new();
    let spacing = spacing.max(1.0);
    // Lines x - y = c, c from -d to w, clipped to the rectangle.
    let mut c = -d + spacing * 0.5;
    while c < w {
        let (x0, x1) = (c.max(0.0), (c + d).min(w));
        if x1 - x0 > 0.5 {
            let (y0, y1) = (x0 - c, x1 - c);
            out.push((frame.at(x0 - w * 0.5, y0), frame.at(x1 - w * 0.5, y1)));
        }
        c += spacing;
    }
    out
}

fn x_lines(poly: &[Point]) -> [(Point, Point); 2] {
    [(poly[0], poly[2]), (poly[1], poly[3])]
}

/// A plan dimension string: the line, a tick at each end and the text above
/// the middle.
fn draw_dimension(
    painter: &egui::Painter,
    cam: &Camera,
    a: &Point,
    b: &Point,
    text: &str,
    stroke: Stroke,
) {
    let (pa, pb) = (sc(cam, *a), sc(cam, *b));
    painter.line_segment([pa, pb], stroke);
    let d = pb - pa;
    if d.length() > 1.0 {
        let n = egui::vec2(-d.y, d.x).normalized() * 4.0;
        for p in [pa, pb] {
            painter.line_segment([p - n, p + n], stroke);
        }
    }
    painter.text(
        pa + d * 0.5,
        egui::Align2::CENTER_BOTTOM,
        text,
        egui::FontId::proportional(10.0),
        stroke.color,
    );
}

/// Draws the parts of the fireplaces of the active floor that the symbol
/// outline lacks and the chimney chases passing through this floor. Called
/// from `render::draw_plan` after the walls, so a fireplace built into a wall
/// shows over the wall lines; the decking and level steps of [`deck`] are
/// drawn in the pass before the walls (`roof_view::draw_roofs`).
pub fn draw(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let pal = &cx.palette;
    let floor = cx.floor();
    let ink = Stroke::new(1.2_f32, pal.text);
    let fine = Stroke::new(0.8_f32, pal.text.gamma_multiply(0.7));
    let faint = Stroke::new(0.8_f32, pal.text.gamma_multiply(0.45));
    for (sym, fp) in floor.fireplace_symbols() {
        if !cx.layers().is_visible(&sym.layer) {
            continue;
        }
        let frame = Frame::of(sym);
        let body = body_poly(sym);
        if fp.kind == FireplaceKind::Masonry {
            for (a, b) in hatch(&frame, sym.width, sym.depth, 6.0) {
                painter.line_segment([sc(cam, a), sc(cam, b)], faint);
            }
        } else {
            painter.add(Shape::convex_polygon(
                closed(cam, &body),
                pal.text.gamma_multiply(0.06),
                Stroke::NONE,
            ));
        }
        if fp.kind.has_firebox() && !fp.no_firebox {
            let fb = firebox_poly(&fp, sym);
            painter.add(Shape::convex_polygon(
                closed(cam, &fb),
                Color32::from_rgba_unmultiplied(40, 30, 25, 70),
                ink,
            ));
            let hearth = hearth_poly(&fp, sym);
            if hearth.len() >= 3 {
                painter.add(Shape::convex_polygon(
                    closed(cam, &hearth),
                    pal.text.gamma_multiply(0.07),
                    Stroke::NONE,
                ));
                dashed(painter, cam, &hearth, ink);
            }
            let shelf = mantel_poly(&fp, sym);
            if shelf.len() >= 3 {
                dashed(painter, cam, &shelf, fine);
            }
        }
        // The rough opening around a fireplace built into a wall.
        if fp.rough.show_in_plan {
            let rough = rough_poly(&fp, sym);
            if rough.len() >= 3 {
                dashed(painter, cam, &rough, fine);
            }
        }
        // Its width and firebox width (Suppress Dimensions turns them off).
        for dim in plan_dimensions(&fp, sym) {
            draw_dimension(painter, cam, &dim.from, &dim.to, &dim.text, fine);
        }
        if fp.chimney.enabled {
            let chimney = chimney_poly(&fp, sym);
            if fp.kind.has_firebox() {
                dashed(painter, cam, &chimney, fine);
            }
            for (a, b) in x_lines(&chimney) {
                painter.line_segment([sc(cam, a), sc(cam, b)], fine);
            }
        }
    }
    // The chases of fireplaces on the floors below.
    if cx.layers().is_visible(FIREPLACE_LAYER) {
        for (_, poly) in chases_on_floor(&cx.project, cx.floor) {
            dashed(painter, cam, &poly, ink);
            for (a, b) in x_lines(&poly) {
                painter.line_segment([sc(cam, a), sc(cam, b)], fine);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::WallKind;

    fn cx_with_wall() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut w = Wall::new(
            Point::new(0.0, 0.0),
            Point::new(300.0, 0.0),
            6.5,
            109.125,
            WallKind::Exterior,
        );
        w.id = cx.project.alloc_id();
        cx.floor_mut().walls.push(w);
        cx
    }

    #[test]
    fn near_a_wall_the_back_goes_flush_to_the_face_facing_the_pointer() {
        let cx = cx_with_wall();
        // Pointer above the wall (y > 0): the room is on the +y side.
        let pl = placement_at(
            cx.floor(),
            Point::new(150.0, 20.0),
            FireplaceKind::Masonry,
            false,
            0.0,
        );
        assert!(pl.wall.is_some() && !pl.in_wall);
        assert!(pl.angle.abs() < 1e-9, "front faces +y: {}", pl.angle);
        assert!((pl.position.x - 150.0).abs() < 1e-9);
        assert!((pl.position.y - 3.25).abs() < 1e-9, "{:?}", pl.position);
        // Below the wall the fireplace turns round.
        let below = placement_at(
            cx.floor(),
            Point::new(150.0, -20.0),
            FireplaceKind::Masonry,
            false,
            0.0,
        );
        assert!((below.angle.abs() - 180.0).abs() < 1e-9, "{}", below.angle);
        assert!((below.position.y + 3.25).abs() < 1e-9);
    }

    #[test]
    fn built_into_the_wall_the_back_is_flush_with_the_outside_face() {
        let cx = cx_with_wall();
        let pl = placement_at(
            cx.floor(),
            Point::new(150.0, 20.0),
            FireplaceKind::Masonry,
            true,
            0.0,
        );
        assert!(pl.in_wall);
        // Facing +y: the back is flush with the face at y = -3.25 and the 24
        // in body stands 20 3/4 in out into the room (manual p. 757).
        assert!((pl.position.y + 3.25).abs() < 1e-9, "{:?}", pl.position);
        assert!(pl.angle.abs() < 1e-9);
    }

    /// A closed 300 x 200 room of exterior walls, with an interior partition.
    fn cx_with_room() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(300.0, 0.0),
            Point::new(300.0, 200.0),
            Point::new(0.0, 200.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.5, 109.125, WallKind::Exterior);
        }
        cx.refresh();
        cx
    }

    #[test]
    fn on_an_exterior_wall_the_fireplace_faces_the_interior_whichever_side_is_clicked() {
        let cx = cx_with_room();
        for in_wall in [false, true] {
            // Outside the bottom wall (y < 0): still faces +y, into the room.
            let outside = placement_at(
                cx.floor(),
                Point::new(150.0, -20.0),
                FireplaceKind::Masonry,
                in_wall,
                0.0,
            );
            assert!(outside.wall.is_some());
            assert!(outside.angle.abs() < 1e-9, "{in_wall}: {}", outside.angle);
            // Inside the room: the same.
            let inside = placement_at(
                cx.floor(),
                Point::new(150.0, 20.0),
                FireplaceKind::Masonry,
                in_wall,
                0.0,
            );
            assert!(inside.angle.abs() < 1e-9);
            // The top wall faces -y.
            let top = placement_at(
                cx.floor(),
                Point::new(150.0, 220.0),
                FireplaceKind::Masonry,
                in_wall,
                0.0,
            );
            assert!((top.angle.abs() - 180.0).abs() < 1e-9, "{}", top.angle);
        }
    }

    #[test]
    fn on_an_interior_wall_it_faces_the_edge_clicked() {
        let mut cx = cx_with_room();
        cx.project.add_wall(
            0,
            Point::new(150.0, 0.0),
            Point::new(150.0, 200.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        cx.refresh();
        let right = placement_at(
            cx.floor(),
            Point::new(160.0, 100.0),
            FireplaceKind::Masonry,
            true,
            0.0,
        );
        let left = placement_at(
            cx.floor(),
            Point::new(140.0, 100.0),
            FireplaceKind::Masonry,
            true,
            0.0,
        );
        // The front of a fireplace at angle a points along (-sin a, cos a).
        let front_x = |a: f64| -a.to_radians().sin();
        assert!(
            front_x(right.angle) > 0.99,
            "the right-hand click faces +x: {}",
            right.angle
        );
        assert!(
            front_x(left.angle) < -0.99,
            "the left-hand click faces -x: {}",
            left.angle
        );
    }

    #[test]
    fn the_depth_handle_slides_the_fireplace_out_to_the_inside_edge() {
        let mut cx = cx_with_room();
        let pl = placement_at(
            cx.floor(),
            Point::new(150.0, 20.0),
            FireplaceKind::Masonry,
            true,
            0.0,
        );
        let id = place(&mut cx, FireplaceKind::Masonry, pl);
        let sym = cx.floor().symbol(id).unwrap().clone();
        let start = Point::new(150.0, sym.position.y + sym.depth);
        // Part way out.
        let part =
            slide_in_wall(cx.floor(), &sym, start, Point::new(150.0, start.y - 5.0)).unwrap();
        assert!((part.position.y - (sym.position.y - 5.0)).abs() < 1e-9);
        // Dragged far out it stops with the front on the inside edge (y = 3.25).
        let far = slide_in_wall(cx.floor(), &sym, start, Point::new(150.0, -500.0)).unwrap();
        assert!(
            (far.position.y + far.depth - 3.25).abs() < 1e-9,
            "{}",
            far.position.y + far.depth
        );
        // It never slides into the room past where it started.
        let back = slide_in_wall(cx.floor(), &sym, start, Point::new(150.0, 500.0)).unwrap();
        assert_eq!(back.position, sym.position);
        // Not built in: nothing slides.
        let free = placement_at(
            cx.floor(),
            Point::new(150.0, 120.0),
            FireplaceKind::Masonry,
            false,
            0.0,
        );
        let id2 = place(&mut cx, FireplaceKind::Masonry, free);
        let s2 = cx.floor().symbol(id2).unwrap().clone();
        assert!(slide_in_wall(cx.floor(), &s2, Point::ZERO, Point::new(0.0, -9.0)).is_none());
    }

    #[test]
    fn the_body_stays_on_the_wall_and_free_placement_centers_on_the_pointer() {
        let cx = cx_with_wall();
        // Near the end of the wall: the 72" body is pulled onto it.
        let pl = placement_at(
            cx.floor(),
            Point::new(10.0, 20.0),
            FireplaceKind::Masonry,
            false,
            0.0,
        );
        assert!((pl.position.x - 36.0).abs() < 1e-9);
        // Far from any wall: free, centered on the pointer, any angle.
        let free = placement_at(
            cx.floor(),
            Point::new(150.0, 200.0),
            FireplaceKind::Masonry,
            true,
            90.0,
        );
        assert!(free.wall.is_none() && !free.in_wall);
        assert_eq!(free.angle, 90.0);
        // Facing -x at 90 degrees: the body's middle is at the pointer.
        let sym = {
            let mut s = PlacedSymbol::new("x", free.position, 72.0, 24.0, 96.0);
            s.angle = free.angle;
            s
        };
        let mid = Frame::of(&sym).at(0.0, 12.0);
        assert!(mid.dist(Point::new(150.0, 200.0)) < 1e-9);
    }

    #[test]
    fn railings_and_deck_edges_do_not_host_a_fireplace() {
        let mut cx = cx_with_wall();
        cx.floor_mut().walls[0].class = WallClass::DeckEdge;
        let pl = placement_at(
            cx.floor(),
            Point::new(150.0, 20.0),
            FireplaceKind::Masonry,
            false,
            0.0,
        );
        assert!(pl.wall.is_none());
    }

    #[test]
    fn placing_is_one_undo_step_and_selects_the_symbol() {
        let mut cx = cx_with_wall();
        let pl = placement_at(
            cx.floor(),
            Point::new(150.0, 20.0),
            FireplaceKind::Masonry,
            true,
            0.0,
        );
        let id = place(&mut cx, FireplaceKind::Masonry, pl);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Symbol(id)));
        assert!(is_fireplace(cx.floor(), id));
        let (sym, fp) = load(cx.floor(), id).unwrap();
        assert!(fp.in_wall);
        assert_eq!(sym.layer, FIREPLACE_LAYER);
        assert!(cx.project.layers.get(FIREPLACE_LAYER).is_some());
        assert_eq!(sym.label, "Fireplace");
        cx.undo();
        assert!(cx.floor().fireplaces.is_empty());
        assert!(cx.floor().symbols.is_empty());
        cx.redo();
        assert_eq!(cx.floor().fireplace_symbols().len(), 1);
    }

    #[test]
    fn applying_the_specification_is_one_undo_step() {
        let mut cx = cx_with_wall();
        let id = place(
            &mut cx,
            FireplaceKind::Masonry,
            Placement {
                position: Point::new(150.0, 100.0),
                angle: 0.0,
                wall: None,
                in_wall: false,
            },
        );
        let (mut sym, mut fp) = load(cx.floor(), id).unwrap();
        fp.name = "Great Room Hearth".into();
        fp.mantel.height = 60.0;
        fp.label.show = false;
        sym.width = 80.0;
        assert!(apply(&mut cx, &sym, &fp));
        let (sym, fp) = load(cx.floor(), id).unwrap();
        assert_eq!(fp.name, "Great Room Hearth");
        assert_eq!(fp.mantel.height, 60.0);
        assert_eq!(sym.width, 80.0);
        assert_eq!(sym.label, " ");
        cx.undo();
        let (sym, fp) = load(cx.floor(), id).unwrap();
        assert_eq!(fp.name, "Fireplace");
        assert_eq!(sym.width, 72.0);
    }

    #[test]
    fn deleting_the_symbol_leaves_no_record() {
        let mut cx = cx_with_wall();
        let id = place(
            &mut cx,
            FireplaceKind::Prefab,
            Placement {
                position: Point::new(150.0, 100.0),
                angle: 0.0,
                wall: None,
                in_wall: false,
            },
        );
        assert_eq!(cx.floor().fireplaces.len(), 1);
        cx.selection.set(ObjectRef::Symbol(id));
        cx.delete_selection();
        assert!(cx.floor().symbols.is_empty());
        assert!(cx.floor().fireplaces.is_empty());
    }

    #[test]
    fn picking_finds_the_body_the_hearth_and_the_mantel() {
        let mut cx = cx_with_wall();
        let id = place(
            &mut cx,
            FireplaceKind::Masonry,
            Placement {
                position: Point::new(150.0, 100.0),
                angle: 0.0,
                wall: None,
                in_wall: false,
            },
        );
        let f = cx.floor();
        assert_eq!(hit(f, Point::new(150.0, 112.0), 1.0), Some(id));
        // The hearth is in front of the body (y 124..140).
        assert_eq!(hit(f, Point::new(150.0, 132.0), 1.0), Some(id));
        assert_eq!(hit(f, Point::new(150.0, 300.0), 1.0), None);
    }

    #[test]
    fn chimneys_cut_holes_in_the_roof_with_clearance() {
        let mut cx = cx_with_wall();
        place(
            &mut cx,
            FireplaceKind::Masonry,
            Placement {
                position: Point::new(150.0, 100.0),
                angle: 0.0,
                wall: None,
                in_wall: false,
            },
        );
        let holes = chimney_holes(&cx.project);
        assert_eq!(holes.len(), 1);
        // 48 x 24 shaft grown by 2" each side: 52 x 28.
        let area = plan_core::geometry::polygon_area(&holes[0].1).abs();
        assert!((area - 52.0 * 28.0).abs() < 1e-6, "{area}");
    }

    #[test]
    fn the_edit_toolbar_offers_the_specification_for_a_fireplace_only() {
        let mut cx = cx_with_wall();
        assert!(edit_actions(&cx).is_empty());
        let id = place(
            &mut cx,
            FireplaceKind::Masonry,
            Placement {
                position: Point::new(150.0, 100.0),
                angle: 0.0,
                wall: None,
                in_wall: false,
            },
        );
        let actions = edit_actions(&cx);
        assert!(actions.iter().any(|a| matches!(
            a.kind,
            EditActionKind::Custom {
                id: cmd::OPEN_SPEC,
                ..
            }
        )));
        assert!(run_command(&mut cx, cmd::OPEN_SPEC));
        assert_eq!(take_open(), Some(id));
        assert!(cx
            .requests
            .contains(&EditorRequest::SetTool(ToolId::Fireplace)));
        assert!(!run_command(&mut cx, "something.else"));
    }

    #[test]
    fn the_brick_hatch_stays_inside_the_body() {
        let sym = PlacedSymbol::new("x", Point::new(0.0, 0.0), 72.0, 24.0, 96.0);
        let frame = Frame::of(&sym);
        let lines = hatch(&frame, 72.0, 24.0, 6.0);
        assert!(lines.len() > 10);
        for (a, b) in lines {
            for p in [a, b] {
                assert!(p.x >= -36.0 - 1e-9 && p.x <= 36.0 + 1e-9);
                assert!(p.y >= -1e-9 && p.y <= 24.0 + 1e-9);
            }
        }
    }
}
