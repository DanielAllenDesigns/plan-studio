//! Compound 3D solids (the result of Union, Subtract and Intersect) and
//! architectural blocks in the plan view (reference manual "Architectural
//! Blocks" and "3D Solid Tools", pp. 1057 to 1081).
//!
//! A compound solid is `ObjectRef::Solid(id)`: it picks by its plan outline,
//! moves, turns and mirrors as one, and deletes. A block is drawn as a dashed
//! bounding box with its label; its members draw themselves. See
//! `plan_core::solids` and `plan_core::arch_block`.

use super::{Camera, EditorContext, ObjectRef};
use crate::tools::arch_block;
use eframe::egui::{self, Align2, Color32, FontId, Stroke};
use plan_3d::triangulate::ear_clip;
use plan_core::arch_block::{BLOCK_LABEL_LAYER, BLOCK_LAYER};
use plan_core::details::SOLID_LAYER;
use plan_core::geometry::{dist_to_segment, point_in_polygon, Point};
use plan_core::solids::CompoundSolid;
use plan_core::{Floor, Id, Layer, LayerSet, LineStyle};

/// The layers blocks and compound solids use, with Chief-like colors.
fn layer_defaults() -> [Layer; 3] {
    [
        Layer::new(BLOCK_LAYER, [120, 80, 160], 18),
        Layer::new(BLOCK_LABEL_LAYER, [120, 80, 160], 18),
        Layer::new(SOLID_LAYER, [90, 90, 90], 18),
    ]
}

/// Adds the layers when the plan lacks them.
pub fn ensure_layers(set: &mut LayerSet) {
    for l in layer_defaults() {
        if set.get(&l.name).is_none() {
            set.layers.push(l);
        }
    }
}

fn visible(cx: &EditorContext, layer: &str) -> bool {
    cx.layers().is_visible(layer)
}

fn color(cx: &EditorContext, layer: &str, fallback: [u8; 3]) -> Color32 {
    let [r, g, b] = cx.layers().get(layer).map_or(fallback, |l| l.color);
    Color32::from_rgb(r, g, b)
}

/// The plan outline rings of compound solid `id`.
pub fn outline_rings(floor: &Floor, id: Id) -> Vec<Vec<Point>> {
    floor
        .solid_layer
        .compound(id)
        .map(CompoundSolid::outline)
        .unwrap_or_default()
}

/// Every outline point of compound solid `id` (selection bounds, handles).
pub fn outline_points(floor: &Floor, id: Id) -> Vec<Point> {
    outline_rings(floor, id).into_iter().flatten().collect()
}

/// The compound solids under `p`, newest first.
pub fn pick(cx: &EditorContext, p: Point, tol: f64) -> Vec<ObjectRef> {
    let floor = cx.floor();
    let mut out = Vec::new();
    for c in floor.solid_layer.compounds.iter().rev() {
        if !visible(cx, &c.layer) {
            continue;
        }
        let hit = c.outline().iter().any(|ring| {
            ring.len() >= 3
                && (point_in_polygon(p, ring)
                    || (0..ring.len())
                        .any(|i| dist_to_segment(p, ring[i], ring[(i + 1) % ring.len()]) <= tol))
        });
        if hit {
            out.push(ObjectRef::Solid(c.id));
        }
    }
    out
}

/// The compound solids whose plan box lies in (or, with `crossing`, touches)
/// the marquee `lo..hi`.
pub fn in_rect(cx: &EditorContext, lo: Point, hi: Point, crossing: bool) -> Vec<ObjectRef> {
    let mut out = Vec::new();
    for c in &cx.floor().solid_layer.compounds {
        if !visible(cx, &c.layer) {
            continue;
        }
        let Some((a, b)) = c.plan_bounds() else {
            continue;
        };
        let inside = a.x >= lo.x && a.y >= lo.y && b.x <= hi.x && b.y <= hi.y;
        let touches = a.x <= hi.x && b.x >= lo.x && a.y <= hi.y && b.y >= lo.y;
        if (crossing && touches) || inside {
            out.push(ObjectRef::Solid(c.id));
        }
    }
    out
}

/// Moves the compound solids `ids` by `d`.
pub fn translate_ids(cx: &mut EditorContext, ids: &[Id], d: Point) -> usize {
    let fl = cx.floor;
    let mut n = 0;
    for id in ids {
        if let Some(c) = cx.project.floors[fl].solid_layer.compound_mut(*id) {
            c.translate(d);
            n += 1;
        }
    }
    n
}

/// Deletes the compound solids `ids` (the selection forgets them).
pub fn delete_ids(cx: &mut EditorContext, ids: &[Id]) -> usize {
    let fl = cx.floor;
    let mut n = 0;
    for id in ids {
        if cx.project.floors[fl].solid_layer.remove_compound(*id) {
            n += 1;
        }
    }
    cx.selection
        .items
        .retain(|o| !matches!(o, ObjectRef::Solid(i) if ids.contains(i)));
    if n > 0 {
        cx.project.floors[fl].prune_blocks(|m| match m {
            plan_core::ObjectRef::Solid(i) => !ids.contains(&i),
            _ => true,
        });
        cx.mark_dirty();
    }
    n
}

/// Compound solids and the boxes and labels of architectural blocks.
pub fn draw(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let floor = cx.floor();
    for c in &floor.solid_layer.compounds {
        if !visible(cx, &c.layer) {
            continue;
        }
        let ink = c.style.color.map_or_else(
            || color(cx, &c.layer, [90, 90, 90]),
            |[r, g, b]| Color32::from_rgb(r, g, b),
        );
        for ring in c.outline() {
            if ring.len() < 3 {
                continue;
            }
            let mut mesh = egui::Mesh::default();
            for p in &ring {
                mesh.colored_vertex(cam.world_to_screen(*p), ink.gamma_multiply(0.12));
            }
            for [a, b, d] in ear_clip(&ring) {
                mesh.add_triangle(a as u32, b as u32, d as u32);
            }
            painter.add(egui::Shape::mesh(mesh));
            crate::editor::details_view::stroke_line(
                painter,
                cam,
                &ring,
                true,
                Stroke::new(1.3_f32, ink),
                LineStyle::Dashed,
            );
        }
    }
    for b in &floor.blocks.blocks {
        if !visible(cx, &b.layer) {
            continue;
        }
        let Some((lo, hi)) = arch_block::block_bounds(cx, b.id) else {
            continue;
        };
        let ink = color(cx, &b.layer, [120, 80, 160]);
        if b.display_bounding_box {
            let ring = [lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)];
            crate::editor::details_view::stroke_line(
                painter,
                cam,
                &ring,
                true,
                Stroke::new(1.0_f32, ink),
                LineStyle::DashDot,
            );
        }
        if visible(cx, BLOCK_LABEL_LAYER) {
            if let Some(text) = b.label_text().filter(|t| !t.is_empty()) {
                let at = cam.world_to_screen(Point::new(lo.x, hi.y) + b.label.offset);
                painter.text(
                    at + egui::vec2(2.0, -2.0),
                    Align2::LEFT_BOTTOM,
                    text,
                    FontId::proportional(11.0),
                    ink,
                );
            }
        }
    }
}

/// The highlight of a selected compound solid or block box.
pub fn highlight(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    o: ObjectRef,
    stroke: Stroke,
) {
    match o {
        ObjectRef::Solid(id) => {
            for ring in outline_rings(cx.floor(), id) {
                crate::editor::details_view::stroke_line(
                    painter,
                    cam,
                    &ring,
                    true,
                    stroke,
                    LineStyle::Solid,
                );
            }
        }
        ObjectRef::Block(id) => {
            if let Some((lo, hi)) = arch_block::block_bounds(cx, id) {
                let ring = [lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)];
                crate::editor::details_view::stroke_line(
                    painter,
                    cam,
                    &ring,
                    true,
                    stroke,
                    LineStyle::Dashed,
                );
            }
        }
        _ => {}
    }
}
