//! Slabs, slab holes, square pads, round piers and the holes in the floor and
//! ceiling platforms: storage, editing, picking and plan drawing
//! (`plan_core::foundation`, the Slab flyout and the Floor flyout).
//!
//! # Storage
//!
//! A floor's [`FoundationLayer`] lives in the typed slot `Floor.foundation`
//! (`plan_core::foundation`); [`save`] also makes sure the layers the objects
//! are drawn on exist in the plan. Undo and redo restore it with the rest of
//! the project.
//!
//! # Selection
//!
//! The Select tool and the foundation tools share `cx.selection`: an object is
//! `ObjectRef::Foundation(id)` ([`pick`] finds it, [`in_rect_or_touching`]
//! box-selects, [`translate_ids`] moves, [`delete_ids`] deletes, [`selected`]
//! and [`select`] read and write the selection for the tools). Slabs, slab
//! holes and platform holes have vertex handles ([`outline_points`],
//! [`move_vertex`]).
//!
//! # Drawing
//!
//! [`draw_foundation`] is called from `render::draw_plan` (under the walls);
//! the foundation tools do not draw the objects themselves.

use super::{Camera, EditorContext};
use eframe::egui::{self, Color32, Pos2, Shape, Stroke};
use plan_3d::triangulate::ear_clip_with_holes;
use plan_core::foundation::{
    self, bounds, offset_ring, FoundationLayer, FoundationRef, Pad, PlatformHole, PlatformKind,
    Slab, SlabHole, CEILING_HOLES_LAYER, FLOOR_HOLES_LAYER, PIER_LAYER, SLAB_LAYER,
};
use plan_core::geometry::{dist_to_segment, point_in_polygon, polygon_area, Point};
use plan_core::{Id, Layer, LayerSet, LineStyle, Project};

pub use foundation::{Footing, Pier};

/// Plan-view layer of the slab objects.
pub const LAYER_SLABS: &str = SLAB_LAYER;
/// Smallest outline area a drawn shape needs, square inches.
pub const MIN_AREA: f64 = 1.0;
/// Hatch line spacing in the plan, inches.
const HATCH_SPACING: f64 = 12.0;

/// The layers the foundation objects use, with Chief-like colors and weights.
fn layer_defaults() -> [Layer; 4] {
    [
        Layer::new(SLAB_LAYER, [120, 120, 120], 25),
        Layer::new(PIER_LAYER, [100, 100, 100], 25),
        Layer::new(FLOOR_HOLES_LAYER, [160, 80, 80], 18),
        Layer::new(CEILING_HOLES_LAYER, [80, 80, 160], 18),
    ]
}

// ===================================================================
// Storage and editing
// ===================================================================

/// The foundation layer of the active floor.
pub fn load(cx: &EditorContext) -> FoundationLayer {
    FoundationLayer::load(cx.floor())
}

/// Stores `layer` on floor `fi` and adds the layers it needs to the plan.
pub fn save(project: &mut Project, fi: usize, layer: &FoundationLayer) {
    layer.store(&mut project.floors[fi]);
    if !layer.is_empty() {
        ensure_layers(&mut project.layers);
    }
}

/// Adds the foundation layers when the plan lacks them.
pub fn ensure_layers(set: &mut LayerSet) {
    for l in layer_defaults() {
        if set.get(&l.name).is_none() {
            set.layers.push(l);
        }
    }
}

/// Runs `edit` on the active floor's foundation layer as one undo step.
pub fn edit(cx: &mut EditorContext, label: &str, edit: impl FnOnce(&mut FoundationLayer)) {
    cx.begin_change(label);
    let mut layer = load(cx);
    edit(&mut layer);
    let fl = cx.floor;
    save(&mut cx.project, fl, &layer);
    cx.mark_dirty();
}

/// Adds a slab (with the default footing when `footing`); returns its id.
pub fn add_slab(cx: &mut EditorContext, outline: Vec<Point>, footing: bool) -> Id {
    let id = cx.project.alloc_id();
    let label = if footing { "Slab with Footing" } else { "Slab" };
    edit(cx, label, |l| {
        let mut s = Slab::new(id, outline);
        if footing {
            s.footing = Some(Footing::default());
        }
        l.slabs.push(s);
    });
    id
}

/// Adds a hole in the slab floor; returns its id.
pub fn add_slab_hole(cx: &mut EditorContext, outline: Vec<Point>, footing: bool) -> Id {
    let id = cx.project.alloc_id();
    let label = if footing {
        "Slab Hole with Footing"
    } else {
        "Slab Hole"
    };
    edit(cx, label, |l| {
        l.holes.push(SlabHole::new(id, outline, footing));
    });
    id
}

/// Adds a square pad centered on `center`; returns its id.
pub fn add_pad(cx: &mut EditorContext, center: Point) -> Id {
    let id = cx.project.alloc_id();
    edit(cx, "Square Pad", |l| l.pads.push(Pad::new(id, center)));
    id
}

/// Adds a round pier centered on `center`; returns its id.
pub fn add_pier(cx: &mut EditorContext, center: Point) -> Id {
    let id = cx.project.alloc_id();
    edit(cx, "Round Pier", |l| l.piers.push(Pier::new(id, center)));
    id
}

/// Adds a hole in the floor or ceiling platform; returns its id.
pub fn add_platform_hole(cx: &mut EditorContext, outline: Vec<Point>, kind: PlatformKind) -> Id {
    let id = cx.project.alloc_id();
    let label = match kind {
        PlatformKind::Floor => "Hole in Floor Platform",
        PlatformKind::Ceiling => "Hole in Ceiling Platform",
    };
    edit(cx, label, |l| {
        l.platform_holes.push(PlatformHole::new(id, outline, kind));
    });
    id
}

/// Deletes the object as one undo step; returns whether it existed.
pub fn delete(cx: &mut EditorContext, r: FoundationRef) -> bool {
    if load(cx).find(r.id()).is_none() {
        return false;
    }
    edit(cx, &format!("Delete {}", r.name()), |l| {
        l.remove(r);
    });
    forget(cx, &[r.id()]);
    true
}

/// Moves the object by `d` as one undo step; returns whether it existed.
pub fn move_by(cx: &mut EditorContext, r: FoundationRef, d: Point) -> bool {
    if load(cx).find(r.id()).is_none() {
        return false;
    }
    edit(cx, &format!("Move {}", r.name()), |l| {
        l.translate(r, d);
    });
    true
}

/// Deletes the objects with these ids as one undo step; returns how many went.
pub fn delete_ids(cx: &mut EditorContext, ids: &[Id]) -> usize {
    let layer = load(cx);
    let refs: Vec<FoundationRef> = ids.iter().filter_map(|i| layer.find(*i)).collect();
    if refs.is_empty() {
        return 0;
    }
    let label = match refs.as_slice() {
        [one] => format!("Delete {}", one.name()),
        _ => "Delete Foundation Objects".to_string(),
    };
    edit(cx, &label, |l| {
        for r in &refs {
            l.remove(*r);
        }
    });
    forget(cx, ids);
    refs.len()
}

/// Translates the objects with these ids by `d` inside the caller's undo
/// step (group drags and nudges); returns how many moved.
pub fn translate_ids(cx: &mut EditorContext, ids: &[Id], d: Point) -> usize {
    let mut layer = load(cx);
    let mut n = 0;
    for id in ids {
        if let Some(r) = layer.find(*id) {
            n += usize::from(layer.translate(r, d));
        }
    }
    if n > 0 {
        let fl = cx.floor;
        save(&mut cx.project, fl, &layer);
        cx.mark_dirty();
    }
    n
}

/// The outline of a slab, slab hole or platform hole: the objects whose
/// corners can be dragged. `None` for pads and piers.
pub fn outline_points(layer: &FoundationLayer, r: FoundationRef) -> Option<Vec<Point>> {
    match r {
        FoundationRef::Slab(i) => layer.slab(i).map(|s| s.outline.clone()),
        FoundationRef::SlabHole(i) => layer.hole(i).map(|h| h.outline.clone()),
        FoundationRef::PlatformHole(i) => layer.platform_hole(i).map(|h| h.outline.clone()),
        FoundationRef::Pad(_) | FoundationRef::Pier(_) => None,
    }
}

/// Moves corner `i` of the outline of `r` to `to`. Returns whether it moved
/// (the object has an outline with that corner).
pub fn move_vertex_in(layer: &mut FoundationLayer, r: FoundationRef, i: usize, to: Point) -> bool {
    let outline = match r {
        FoundationRef::Slab(id) => layer.slab_mut(id).map(|s| &mut s.outline),
        FoundationRef::SlabHole(id) => layer.hole_mut(id).map(|h| &mut h.outline),
        FoundationRef::PlatformHole(id) => layer.platform_hole_mut(id).map(|h| &mut h.outline),
        FoundationRef::Pad(_) | FoundationRef::Pier(_) => None,
    };
    match outline.and_then(|o| o.get_mut(i)) {
        Some(v) => {
            *v = to;
            true
        }
        None => false,
    }
}

/// Does the active floor have the object?
pub fn exists(cx: &EditorContext, r: FoundationRef) -> bool {
    load(cx).find(r.id()) == Some(r)
}

// ===================================================================
// Selection
// ===================================================================

/// The selected foundation object, if exactly the objects of this module
/// are selected and one of them still exists.
pub fn selected(cx: &EditorContext) -> Option<FoundationRef> {
    let layer = load(cx);
    cx.selection.items.iter().find_map(|o| match o {
        super::ObjectRef::Foundation(id) => layer.find(*id),
        _ => None,
    })
}

/// Selects `r` alone.
pub fn select(cx: &mut EditorContext, r: FoundationRef) {
    cx.selection.set(super::ObjectRef::Foundation(r.id()));
}

/// Drops the foundation objects from the selection.
pub fn clear_selection(cx: &mut EditorContext) {
    cx.selection
        .items
        .retain(|o| !matches!(o, super::ObjectRef::Foundation(_)));
}

/// Drops the objects with these ids from the selection.
fn forget(cx: &mut EditorContext, ids: &[Id]) {
    cx.selection
        .items
        .retain(|o| !matches!(o, super::ObjectRef::Foundation(i) if ids.contains(i)));
}

// ===================================================================
// Picking
// ===================================================================

fn near_edge(outline: &[Point], p: Point, tol: f64) -> bool {
    let n = outline.len();
    n >= 2 && (0..n).any(|i| dist_to_segment(p, outline[i], outline[(i + 1) % n]) <= tol)
}

fn visible(cx: &EditorContext, layer: &str) -> bool {
    cx.layers().is_visible(layer)
}

/// The object at `p` within `tol` inches: piers and pads first, then the
/// platform holes and slab holes by their edges, then slabs by their edge or
/// inside (the smallest slab first).
pub fn pick(cx: &EditorContext, p: Point, tol: f64) -> Option<FoundationRef> {
    let layer = load(cx);
    for pier in &layer.piers {
        if visible(cx, &pier.layer) && pier.center.dist(p) <= pier.diameter * 0.5 + tol {
            return Some(FoundationRef::Pier(pier.id));
        }
    }
    for pad in &layer.pads {
        let b = pad.solid();
        if visible(cx, &pad.layer)
            && p.x >= b.min.x - tol
            && p.x <= b.max.x + tol
            && p.y >= b.min.y - tol
            && p.y <= b.max.y + tol
        {
            return Some(FoundationRef::Pad(pad.id));
        }
    }
    for h in &layer.platform_holes {
        if visible(cx, h.layer()) && near_edge(&h.outline, p, tol) {
            return Some(FoundationRef::PlatformHole(h.id));
        }
    }
    for h in &layer.holes {
        if visible(cx, &h.layer) && near_edge(&h.outline, p, tol) {
            return Some(FoundationRef::SlabHole(h.id));
        }
    }
    let mut slabs: Vec<&Slab> = layer
        .slabs
        .iter()
        .filter(|s| {
            visible(cx, &s.layer)
                && (near_edge(&s.outline, p, tol) || point_in_polygon(p, &s.outline))
        })
        .collect();
    slabs.sort_by(|a, b| a.gross_area().total_cmp(&b.gross_area()));
    slabs.first().map(|s| FoundationRef::Slab(s.id))
}

/// Was `r` (a slab) picked only because `p` lies inside it, not near its edge?
/// Interior picks rank below rooms.
pub fn picked_by_interior(cx: &EditorContext, r: FoundationRef, p: Point, tol: f64) -> bool {
    match r {
        FoundationRef::Slab(id) => load(cx)
            .slab(id)
            .is_some_and(|s| !near_edge(&s.outline, p, tol)),
        _ => false,
    }
}

/// Every object whose plan outline lies inside the box `lo`..`hi`.
pub fn in_rect(cx: &EditorContext, lo: Point, hi: Point) -> Vec<FoundationRef> {
    in_rect_or_touching(cx, lo, hi, false)
}

/// The objects inside the box `lo`..`hi`, or, with `crossing`, those whose
/// bounding box touches it.
pub fn in_rect_or_touching(
    cx: &EditorContext,
    lo: Point,
    hi: Point,
    crossing: bool,
) -> Vec<FoundationRef> {
    let layer = load(cx);
    let inside = |pts: &[Point]| {
        !pts.is_empty() && {
            let (a, b) = bounds(pts);
            if crossing {
                a.x <= hi.x && b.x >= lo.x && a.y <= hi.y && b.y >= lo.y
            } else {
                a.x >= lo.x && a.y >= lo.y && b.x <= hi.x && b.y <= hi.y
            }
        }
    };
    let mut out = Vec::new();
    out.extend(
        layer
            .slabs
            .iter()
            .filter(|s| visible(cx, &s.layer) && inside(&s.outline))
            .map(|s| FoundationRef::Slab(s.id)),
    );
    out.extend(
        layer
            .holes
            .iter()
            .filter(|s| visible(cx, &s.layer) && inside(&s.outline))
            .map(|s| FoundationRef::SlabHole(s.id)),
    );
    out.extend(
        layer
            .pads
            .iter()
            .filter(|s| visible(cx, &s.layer) && inside(&s.outline()))
            .map(|s| FoundationRef::Pad(s.id)),
    );
    out.extend(
        layer
            .piers
            .iter()
            .filter(|s| {
                let r = s.diameter * 0.5;
                visible(cx, &s.layer)
                    && inside(&[
                        Point::new(s.center.x - r, s.center.y - r),
                        Point::new(s.center.x + r, s.center.y + r),
                    ])
            })
            .map(|s| FoundationRef::Pier(s.id)),
    );
    out.extend(
        layer
            .platform_holes
            .iter()
            .filter(|s| visible(cx, s.layer()) && inside(&s.outline))
            .map(|s| FoundationRef::PlatformHole(s.id)),
    );
    out
}

// ===================================================================
// Plan drawing
// ===================================================================

fn sc(cam: &Camera, p: Point) -> Pos2 {
    cam.world_to_screen(p)
}

fn layer_color(cx: &EditorContext, name: &str, fallback: [u8; 3]) -> Color32 {
    let [r, g, b] = cx.layers().get(name).map_or(fallback, |l| l.color);
    Color32::from_rgb(r, g, b)
}

/// A world outline in `stroke`, optionally closed and dashed (the tools'
/// rubber band).
pub fn draw_outline(
    painter: &egui::Painter,
    cam: &Camera,
    pts: &[Point],
    closed: bool,
    stroke: Stroke,
    dashed: bool,
) {
    let style = if dashed {
        LineStyle::Dashed
    } else {
        LineStyle::Solid
    };
    stroke_line(painter, cam, pts, closed, stroke, style);
}

fn stroke_line(
    painter: &egui::Painter,
    cam: &Camera,
    pts: &[Point],
    closed: bool,
    stroke: Stroke,
    style: LineStyle,
) {
    if pts.len() < 2 {
        return;
    }
    let mut screen: Vec<Pos2> = pts.iter().map(|p| sc(cam, *p)).collect();
    if closed {
        screen.push(screen[0]);
    }
    match style {
        LineStyle::Solid => {
            painter.add(Shape::line(screen, stroke));
        }
        LineStyle::Dashed => painter.extend(Shape::dashed_line(&screen, stroke, 9.0, 5.0)),
        LineStyle::Dotted => painter.extend(Shape::dashed_line(&screen, stroke, 2.0, 4.0)),
        LineStyle::DashDot => painter.extend(Shape::dashed_line(&screen, stroke, 10.0, 8.0)),
    }
}

/// Fills `outline` minus `holes` with a flat color.
fn fill_polygon(
    painter: &egui::Painter,
    cam: &Camera,
    outline: &[Point],
    holes: &[Vec<Point>],
    color: Color32,
) {
    if outline.len() < 3 || color.a() == 0 {
        return;
    }
    let mut all: Vec<Point> = outline.to_vec();
    for h in holes {
        all.extend_from_slice(h);
    }
    let mut mesh = egui::Mesh::default();
    for p in &all {
        mesh.colored_vertex(sc(cam, *p), color);
    }
    for [a, b, c] in ear_clip_with_holes(outline, holes) {
        mesh.add_triangle(a as u32, b as u32, c as u32);
    }
    painter.add(Shape::mesh(mesh));
}

/// Hatch lines at `angle_deg` across `outline` minus `holes`: each scan line
/// is cut at every edge and drawn between alternate crossings.
fn hatch_polygon(
    painter: &egui::Painter,
    cam: &Camera,
    outline: &[Point],
    holes: &[Vec<Point>],
    angle_deg: f64,
    color: Color32,
) {
    let (lo, hi) = bounds(outline);
    let spacing = HATCH_SPACING.max(5.0 / cam.px_per_in.max(1e-6));
    let (sin, cos) = angle_deg.to_radians().sin_cos();
    // Rotate into the hatch frame, where lines are horizontal.
    let rot = |p: Point| Point::new(p.x * cos + p.y * sin, -p.x * sin + p.y * cos);
    let unrot = |p: Point| Point::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos);
    let rings: Vec<Vec<Point>> = std::iter::once(outline)
        .chain(holes.iter().map(Vec::as_slice))
        .map(|r| r.iter().map(|p| rot(*p)).collect())
        .collect();
    let corners = [lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)].map(rot);
    let (y0, y1) = corners
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), c| (a.min(c.y), b.max(c.y)));
    let stroke = Stroke::new(1.0_f32, color);
    let mut y = (y0 / spacing).ceil() * spacing;
    let mut lines = 0;
    while y < y1 && lines < 600 {
        let mut xs: Vec<f64> = Vec::new();
        for ring in &rings {
            let n = ring.len();
            for i in 0..n {
                let (a, b) = (ring[i], ring[(i + 1) % n]);
                if (a.y <= y) != (b.y <= y) {
                    xs.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for i in (0..xs.len().saturating_sub(1)).step_by(2) {
            let a = sc(cam, unrot(Point::new(xs[i], y)));
            let b = sc(cam, unrot(Point::new(xs[i + 1], y)));
            painter.line_segment([a, b], stroke);
        }
        y += spacing;
        lines += 1;
    }
}

fn fill_pattern(
    painter: &egui::Painter,
    cam: &Camera,
    slab: &Slab,
    holes: &[Vec<Point>],
    color: Color32,
) {
    let soft = color.gamma_multiply(0.18);
    let line = color.gamma_multiply(0.55);
    match slab.fill_pattern.as_str() {
        "None" => {}
        "Solid" => fill_polygon(
            painter,
            cam,
            &slab.outline,
            holes,
            color.gamma_multiply(0.5),
        ),
        "Cross Hatch" => {
            fill_polygon(painter, cam, &slab.outline, holes, soft);
            hatch_polygon(painter, cam, &slab.outline, holes, 45.0, line);
            hatch_polygon(painter, cam, &slab.outline, holes, 135.0, line);
        }
        "Grid" => {
            fill_polygon(painter, cam, &slab.outline, holes, soft);
            hatch_polygon(painter, cam, &slab.outline, holes, 0.0, line);
            hatch_polygon(painter, cam, &slab.outline, holes, 90.0, line);
        }
        _ => {
            fill_polygon(painter, cam, &slab.outline, holes, soft);
            hatch_polygon(painter, cam, &slab.outline, holes, 45.0, line);
        }
    }
}

fn draw_slab(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    layer: &FoundationLayer,
    slab: &Slab,
) {
    if slab.outline.len() < 3 {
        return;
    }
    let ink = layer_color(cx, &slab.layer, [120, 120, 120]);
    let fill = Color32::from_rgb(slab.fill_color[0], slab.fill_color[1], slab.fill_color[2]);
    let holes = layer.all_hole_outlines(slab);
    fill_pattern(painter, cam, slab, &holes, fill);
    let stroke = Stroke::new(1.6_f32, ink);
    stroke_line(painter, cam, &slab.outline, true, stroke, slab.line_style);
    for h in &holes {
        stroke_line(
            painter,
            cam,
            h,
            true,
            Stroke::new(1.2_f32, ink),
            LineStyle::Dashed,
        );
    }
    if let Some(f) = slab.footing {
        // The footing runs along the inside of the edge.
        let mut ring = slab.outline.clone();
        if polygon_area(&ring) < 0.0 {
            ring.reverse();
        }
        let inner = offset_ring(&ring, f.width);
        stroke_line(
            painter,
            cam,
            &inner,
            true,
            Stroke::new(1.0_f32, ink.gamma_multiply(0.7)),
            LineStyle::Dashed,
        );
    }
}

fn draw_slab_hole(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    slab_footing: Footing,
    h: &SlabHole,
) {
    let ink = layer_color(cx, &h.layer, [120, 120, 120]);
    stroke_line(
        painter,
        cam,
        &h.outline,
        true,
        Stroke::new(1.4_f32, ink),
        h.line_style,
    );
    if h.with_footing {
        let mut ring = h.outline.clone();
        if polygon_area(&ring) > 0.0 {
            ring.reverse(); // hole rings run clockwise
        }
        let grown = offset_ring(&ring, slab_footing.width);
        stroke_line(
            painter,
            cam,
            &grown,
            true,
            Stroke::new(1.0_f32, ink.gamma_multiply(0.7)),
            LineStyle::Dashed,
        );
    }
}

fn draw_pad(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, pad: &Pad) {
    let ink = layer_color(cx, &pad.layer, [100, 100, 100]);
    let o = pad.outline();
    fill_polygon(painter, cam, &o, &[], ink.gamma_multiply(0.2));
    let stroke = Stroke::new(1.6_f32, ink);
    stroke_line(painter, cam, &o, true, stroke, LineStyle::Solid);
    painter.line_segment([sc(cam, o[0]), sc(cam, o[2])], Stroke::new(1.0_f32, ink));
    painter.line_segment([sc(cam, o[1]), sc(cam, o[3])], Stroke::new(1.0_f32, ink));
}

fn draw_pier(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, pier: &Pier) {
    let ink = layer_color(cx, &pier.layer, [100, 100, 100]);
    let c = sc(cam, pier.center);
    let r = (pier.diameter * 0.5 * cam.px_per_in) as f32;
    painter.circle(c, r, ink.gamma_multiply(0.2), Stroke::new(1.6_f32, ink));
    let s = Stroke::new(1.0_f32, ink);
    painter.line_segment([c - egui::vec2(r, 0.0), c + egui::vec2(r, 0.0)], s);
    painter.line_segment([c - egui::vec2(0.0, r), c + egui::vec2(0.0, r)], s);
    if let Some(b) = pier.footing_solid() {
        stroke_line(
            painter,
            cam,
            &b.corners(),
            true,
            Stroke::new(1.0_f32, ink.gamma_multiply(0.7)),
            LineStyle::Dashed,
        );
    }
}

fn draw_platform_hole(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, h: &PlatformHole) {
    let ink = layer_color(
        cx,
        h.layer(),
        match h.kind {
            PlatformKind::Floor => [160, 80, 80],
            PlatformKind::Ceiling => [80, 80, 160],
        },
    );
    stroke_line(
        painter,
        cam,
        &h.outline,
        true,
        Stroke::new(1.4_f32, ink),
        LineStyle::Dashed,
    );
}

fn draw_selection(
    painter: &egui::Painter,
    cam: &Camera,
    cx: &EditorContext,
    layer: &FoundationLayer,
) {
    let refs: Vec<FoundationRef> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            super::ObjectRef::Foundation(id) => layer.find(*id),
            _ => None,
        })
        .collect();
    let stroke = Stroke::new(2.6_f32, cx.palette.selection);
    for r in refs {
        draw_selected(painter, cam, layer, r, stroke);
    }
}

fn draw_selected(
    painter: &egui::Painter,
    cam: &Camera,
    layer: &FoundationLayer,
    r: FoundationRef,
    stroke: Stroke,
) {
    match r {
        FoundationRef::Slab(i) => {
            if let Some(s) = layer.slab(i) {
                stroke_line(painter, cam, &s.outline, true, stroke, LineStyle::Solid);
            }
        }
        FoundationRef::SlabHole(i) => {
            if let Some(h) = layer.hole(i) {
                stroke_line(painter, cam, &h.outline, true, stroke, LineStyle::Solid);
            }
        }
        FoundationRef::Pad(i) => {
            if let Some(p) = layer.pad(i) {
                stroke_line(painter, cam, &p.outline(), true, stroke, LineStyle::Solid);
            }
        }
        FoundationRef::Pier(i) => {
            if let Some(p) = layer.pier(i) {
                painter.circle_stroke(
                    sc(cam, p.center),
                    (p.diameter * 0.5 * cam.px_per_in) as f32,
                    stroke,
                );
            }
        }
        FoundationRef::PlatformHole(i) => {
            if let Some(h) = layer.platform_hole(i) {
                stroke_line(painter, cam, &h.outline, true, stroke, LineStyle::Solid);
            }
        }
    }
}

/// Draws the active floor's slabs, holes, pads and piers, and the selection.
/// Called from `render::draw_plan`.
pub fn draw_foundation(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let layer = load(cx);
    if layer.is_empty() {
        return;
    }
    for s in &layer.slabs {
        if visible(cx, &s.layer) {
            draw_slab(cx, painter, cam, &layer, s);
        }
    }
    for h in &layer.holes {
        if visible(cx, &h.layer) {
            // A hole with footing uses the footing of the slab it cuts.
            let footing = layer
                .slabs
                .iter()
                .find(|s| foundation::hole_inside(&h.outline, &s.outline))
                .and_then(|s| s.footing)
                .unwrap_or_default();
            draw_slab_hole(cx, painter, cam, footing, h);
        }
    }
    for h in &layer.platform_holes {
        if visible(cx, h.layer()) {
            draw_platform_hole(cx, painter, cam, h);
        }
    }
    for p in &layer.pads {
        if visible(cx, &p.layer) {
            draw_pad(cx, painter, cam, p);
        }
    }
    for p in &layer.piers {
        if visible(cx, &p.layer) {
            draw_pier(cx, painter, cam, p);
        }
    }
    draw_selection(painter, cam, cx, &layer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::foundation::{rect_outline, DATA_LAYER};

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    #[test]
    fn adding_stores_the_layer_and_the_layers_it_needs() {
        let mut cx = cx();
        let id = add_slab(
            &mut cx,
            rect_outline(Point::ZERO, Point::new(240.0, 180.0)),
            true,
        );
        let l = load(&cx);
        assert_eq!(l.slabs.len(), 1);
        assert_eq!(l.slabs[0].id, id);
        assert!(l.slabs[0].footing.is_some());
        for name in [SLAB_LAYER, PIER_LAYER] {
            assert!(cx.project.layers.get(name).is_some(), "{name}");
        }
        assert!(cx.project.layers.get("Foundation, Data").is_none());
        assert!(cx.floor().cad.is_empty(), "the typed slot holds the data");
        assert!(cx.floor().foundation.is_some());
        assert_eq!(cx.undo_label(), Some("Slab with Footing"));
        cx.undo();
        assert!(load(&cx).is_empty());
    }

    #[test]
    fn pick_prefers_small_objects_and_respects_layer_visibility() {
        let mut cx = cx();
        let slab = add_slab(
            &mut cx,
            rect_outline(Point::ZERO, Point::new(240.0, 240.0)),
            false,
        );
        let hole = add_slab_hole(
            &mut cx,
            rect_outline(Point::new(100.0, 100.0), Point::new(140.0, 140.0)),
            false,
        );
        let pad = add_pad(&mut cx, Point::new(400.0, 50.0));
        let pier = add_pier(&mut cx, Point::new(400.0, 200.0));
        let ph = add_platform_hole(
            &mut cx,
            rect_outline(Point::new(20.0, 20.0), Point::new(60.0, 60.0)),
            PlatformKind::Floor,
        );
        assert_eq!(
            pick(&cx, Point::new(120.0, 100.0), 4.0),
            Some(FoundationRef::SlabHole(hole))
        );
        assert_eq!(
            pick(&cx, Point::new(200.0, 200.0), 4.0),
            Some(FoundationRef::Slab(slab))
        );
        assert_eq!(
            pick(&cx, Point::new(405.0, 55.0), 4.0),
            Some(FoundationRef::Pad(pad))
        );
        assert_eq!(
            pick(&cx, Point::new(402.0, 204.0), 4.0),
            Some(FoundationRef::Pier(pier))
        );
        assert_eq!(
            pick(&cx, Point::new(40.0, 20.0), 4.0),
            Some(FoundationRef::PlatformHole(ph))
        );
        assert_eq!(pick(&cx, Point::new(900.0, 900.0), 4.0), None);
        cx.project.layers.set_display(SLAB_LAYER, false);
        cx.mark_dirty();
        cx.refresh();
        assert_eq!(pick(&cx, Point::new(200.0, 200.0), 4.0), None);
        let all = in_rect(&cx, Point::new(-10.0, -10.0), Point::new(600.0, 600.0));
        assert!(!all.contains(&FoundationRef::Slab(slab)));
        assert!(all.contains(&FoundationRef::Pad(pad)));
    }

    #[test]
    fn move_and_delete_are_single_undo_steps() {
        let mut cx = cx();
        let pad = add_pad(&mut cx, Point::new(100.0, 100.0));
        let r = FoundationRef::Pad(pad);
        assert!(move_by(&mut cx, r, Point::new(10.0, -20.0)));
        assert_eq!(load(&cx).pad(pad).unwrap().center, Point::new(110.0, 80.0));
        assert_eq!(cx.undo_label(), Some("Move Square Pad"));
        select(&mut cx, r);
        assert!(delete(&mut cx, r));
        assert!(load(&cx).is_empty());
        assert_eq!(selected(&cx), None);
        assert_eq!(cx.undo().as_deref(), Some("Delete Square Pad"));
        assert!(exists(&cx, r));
        assert_eq!(cx.undo().as_deref(), Some("Move Square Pad"));
        assert_eq!(load(&cx).pad(pad).unwrap().center, Point::new(100.0, 100.0));
        assert!(!delete(&mut cx, FoundationRef::Pad(999)));
    }

    #[test]
    fn old_files_with_the_cad_record_migrate_on_load() {
        use plan_core::cad::{CadItem, CadObject};
        let mut layer = FoundationLayer::default();
        layer.pads.push(Pad::new(77, Point::new(5.0, 5.0)));
        let mut p = Project::new("old");
        p.floors[0].cad.push(CadObject {
            id: 0,
            layer: DATA_LAYER.to_string(),
            item: CadItem::Text {
                pos: Point::ZERO,
                text: format!("FND1:{}", serde_json::to_string(&layer).unwrap()),
                height: 1.0,
                angle: 0.0,
            },
        });
        let mut hidden = Layer::new(DATA_LAYER, [150, 150, 150], 13);
        hidden.display = false;
        p.layers.layers.push(hidden);
        let mut loaded = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert!(crate::editor::site_view::migrate_legacy_storage(
            &mut loaded
        ));
        assert!(loaded.floors[0].cad.is_empty());
        assert!(loaded.floors[0].foundation.is_some());
        assert_eq!(FoundationLayer::load(&loaded.floors[0]), layer);
        assert!(loaded.layers.get(DATA_LAYER).is_none());
        assert!(!crate::editor::site_view::migrate_legacy_storage(
            &mut loaded
        ));
    }

    #[test]
    fn project_json_round_trip_keeps_the_objects() {
        let mut cx = cx();
        add_slab(
            &mut cx,
            rect_outline(Point::ZERO, Point::new(240.0, 180.0)),
            false,
        );
        add_pier(&mut cx, Point::new(10.0, 10.0));
        let json = serde_json::to_string(&cx.project).unwrap();
        let p: Project = serde_json::from_str(&json).unwrap();
        let l = FoundationLayer::load(&p.floors[0]);
        assert_eq!((l.slabs.len(), l.piers.len()), (1, 1));
    }
}
