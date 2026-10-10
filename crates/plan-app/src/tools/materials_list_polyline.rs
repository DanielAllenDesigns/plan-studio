//! The Materials List Polyline tool (Tools > Materials List > Materials List
//! Polyline; manual pp. 1369, 1382-1384): draw a rectangle in plan view that
//! outlines the area a Materials List is made for.
//!
//! The area is a closed CAD polyline of the active floor, so it moves, stretches
//! and takes holes like any polyline; the plan also keeps a
//! `plan_core::materials_data::MaterialsPolyline` record for it (the Included
//! Floors/Categories grid and the Included Objects choice, from the plan's
//! Materials List Polyline Defaults). Selecting it offers the Calculate
//! Materials List edit button; Open Object opens its specification.
//!
//! * Drag, or click two opposite corners, to draw it. One undo step.
//! * Escape drops a corner that was clicked.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::materials_list as ml;
use crate::editor::{Camera, EditorContext, ObjectRef};
use eframe::egui::{self, Key, Stroke};
use plan_core::cad::{CadItem, CadObject};
use plan_core::geometry::Point;
use plan_core::layers::Layer;

/// The layer the polylines are drawn on.
pub const LAYER: &str = "Materials List Polylines";

/// A rectangle smaller than this (inches) is a stray click.
const MIN_SIDE: f64 = 6.0;

#[derive(Default)]
pub struct MaterialsListPolylineTool {
    anchor: Option<Point>,
    hover: Option<Point>,
}

/// Draws the polyline of corners `a` and `b` on the active floor: the CAD
/// polyline and its Materials List record. One undo step. Returns its id.
pub fn add_rect(cx: &mut EditorContext, a: Point, b: Point) -> Option<plan_core::Id> {
    if (a.x - b.x).abs() < MIN_SIDE || (a.y - b.y).abs() < MIN_SIDE {
        return None;
    }
    let fl = cx.floor;
    cx.begin_change("Materials List Polyline");
    if cx.project.layers.get(LAYER).is_none() {
        cx.project
            .layers
            .add(Layer::new(LAYER, [0xD0, 0x6A, 0x1C], 18));
    }
    let id = cx.project.alloc_id();
    let (lo, hi) = (
        Point::new(a.x.min(b.x), a.y.min(b.y)),
        Point::new(a.x.max(b.x), a.y.max(b.y)),
    );
    cx.project.floors[fl].cad.push(CadObject {
        id,
        layer: LAYER.to_string(),
        item: CadItem::Polyline {
            points: vec![lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)],
            closed: true,
        },
    });
    let rec = ml::new_polyline_record(&cx.project, fl, id);
    cx.project.materials.polylines.push(rec);
    cx.mark_dirty();
    cx.selection.set(ObjectRef::Cad(id));
    Some(id)
}

impl MaterialsListPolylineTool {
    fn finish(&mut self, cx: &mut EditorContext, b: Point) -> ToolResult {
        let Some(a) = self.anchor.take() else {
            return ToolResult::ignored();
        };
        match add_rect(cx, a, b) {
            Some(_) => {
                cx.status = "Materials List Polyline drawn; click Calculate Materials List".into();
                let mut r = ToolResult::committed("Materials List Polyline");
                r.switch_to = Some(ToolId::Select);
                r
            }
            None => {
                self.anchor = Some(a);
                cx.status = "Drag farther to draw the area".into();
                ToolResult::consumed()
            }
        }
    }
}

impl Tool for MaterialsListPolylineTool {
    fn id(&self) -> ToolId {
        ToolId::MaterialsPolyline
    }

    fn name(&self) -> &'static str {
        "Materials List Polyline"
    }

    fn hint(&self) -> String {
        "Materials List Polyline: drag, or click two corners, to outline the area".into()
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.anchor = None;
        self.hover = None;
        cx.status = self.hint();
    }

    fn deactivate(&mut self, _cx: &mut EditorContext) {
        self.anchor = None;
        self.hover = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        self.hover = Some(cx.snap_at(p.world, None, p.overrides(), &[]).point);
        ToolResult::consumed()
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let at = cx.snap_at(p.world, None, p.overrides(), &[]).point;
        if self.anchor.is_some() {
            return self.finish(cx, at);
        }
        self.anchor = Some(at);
        self.hover = Some(at);
        ToolResult::consumed()
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        // A drag ends the rectangle where the button is let go; a plain click
        // keeps the first corner for the second click.
        let Some(a) = self.anchor else {
            return ToolResult::ignored();
        };
        let at = cx.snap_at(p.world, None, p.overrides(), &[]).point;
        if (at.x - a.x).abs() >= MIN_SIDE && (at.y - a.y).abs() >= MIN_SIDE {
            return self.finish(cx, at);
        }
        ToolResult::consumed()
    }

    fn key(&mut self, _cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(Key::Escape) && self.anchor.take().is_some() {
            return ToolResult::consumed();
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let (Some(a), Some(b)) = (self.anchor, self.hover) else {
            return;
        };
        let corners = [
            Point::new(a.x, a.y),
            Point::new(b.x, a.y),
            Point::new(b.x, b.y),
            Point::new(a.x, b.y),
        ]
        .map(|p| cam.world_to_screen(p));
        painter.add(egui::Shape::closed_line(
            corners.to_vec(),
            Stroke::new(1.5_f32, cx.palette.selection),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::materials_data::IncludedObjects;

    #[test]
    fn a_rectangle_makes_a_cad_polyline_and_its_record_in_one_undo_step() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let before = cx.project.floors[0].cad.len();
        let id = add_rect(&mut cx, Point::new(10.0, 10.0), Point::new(250.0, 190.0)).unwrap();
        assert_eq!(cx.project.floors[0].cad.len(), before + 1);
        let rec = cx.project.materials.polyline(id).expect("record");
        assert_eq!(rec.spec.objects, IncludedObjects::ByCenter);
        assert!(rec.spec.includes("Framing", 0));
        assert_eq!(
            cx.project.layers.get(LAYER).map(|l| l.name.as_str()),
            Some(LAYER)
        );
        assert!(cx.selection.items.contains(&ObjectRef::Cad(id)));
        cx.undo();
        assert!(cx.project.materials.polylines.is_empty());
        assert_eq!(cx.project.floors[0].cad.len(), before);
    }

    #[test]
    fn a_stray_click_draws_nothing() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        assert!(add_rect(&mut cx, Point::new(0.0, 0.0), Point::new(2.0, 90.0)).is_none());
        assert!(cx.project.materials.polylines.is_empty());
    }
}
