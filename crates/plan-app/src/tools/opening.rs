//! Door and window placement (DW-1..DW-15 in `docs/parity/doors-windows.md`).
//!
//! The opening is centered under the pointer's projection onto the wall and
//! snaps to the grid snap unit (1"); new openings come from the defaults
//! templates (exterior door on exterior walls, interior door otherwise,
//! window). A ghost with temporary dimensions to both wall ends follows the
//! pointer. The tool stays active after a placement.

use super::{PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::place_from_template;
use crate::editor::tempdim::{self, jamb_dims, TempDims};
use crate::editor::{render, Camera, EditorContext, ObjectRef};
use crate::toolbar::ViewFlag;
use eframe::egui;
use plan_core::geometry::{dist_to_segment, project_on_segment, Point};
use plan_core::{Id, OpeningKind};

pub struct OpeningTool {
    kind: OpeningKind,
    hover: Option<(Id, f64)>,
}

impl Default for OpeningTool {
    fn default() -> Self {
        Self {
            kind: OpeningKind::Door,
            hover: None,
        }
    }
}

/// The wall under `p` and the snapped opening center along it.
fn target(cx: &EditorContext, p: Point, alt: bool) -> Option<(Id, f64)> {
    let tol = cx.pick_tol();
    let wall = cx
        .floor()
        .walls
        .iter()
        .filter(|w| cx.layers().is_visible(&w.layer))
        .map(|w| (w, dist_to_segment(p, w.start, w.end)))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(w, _)| w)?;
    let (t, _) = project_on_segment(p, wall.start, wall.end);
    let mut offset = t * wall.length();
    let unit = cx.snap_unit();
    if !alt {
        offset = (offset / unit).round() * unit;
    }
    Some((wall.id, offset))
}

impl Tool for OpeningTool {
    fn id(&self) -> ToolId {
        match self.kind {
            OpeningKind::Door => ToolId::Door,
            OpeningKind::Window => ToolId::Window,
        }
    }

    fn name(&self) -> &'static str {
        match self.kind {
            OpeningKind::Door => "Hinged Door",
            OpeningKind::Window => "Window",
        }
    }

    fn hint(&self) -> String {
        match self.kind {
            OpeningKind::Door => "Door: click on a wall to place a door".into(),
            OpeningKind::Window => "Window: click on a wall to place a window".into(),
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        match id {
            ToolId::Door => self.kind = OpeningKind::Door,
            ToolId::Window => self.kind = OpeningKind::Window,
            _ => {}
        }
    }

    fn deactivate(&mut self, _cx: &mut EditorContext) {
        self.hover = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        self.hover = target(cx, p.world, p.modifiers.alt);
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let Some((wid, center)) = target(cx, p.world, p.modifiers.alt) else {
            return ToolResult::consumed();
        };
        let Some(wall_kind) = cx.floor().wall(wid).map(|w| w.kind) else {
            return ToolResult::consumed();
        };
        let (target_key, template) = cx.opening_template(self.kind, wall_kind);
        let extras = cx.default_opening_extras(target_key);
        let label = match self.kind {
            OpeningKind::Door => "Place Door",
            OpeningKind::Window => "Place Window",
        };
        cx.begin_change(label);
        let fl = cx.floor;
        match place_from_template(&mut cx.project, fl, wid, center, &template) {
            Some(id) => {
                cx.extras.openings.insert(id, extras);
                cx.selection.set(ObjectRef::Wall(wid));
                cx.status.clear();
                cx.mark_dirty();
                ToolResult::committed(label)
            }
            None => {
                cx.cancel_change();
                cx.status = "Opening does not fit there (wall too short or overlap)".into();
                ToolResult::consumed()
            }
        }
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let Some((wid, center)) = self.hover else {
            return;
        };
        let Some(wall) = cx.floor().wall(wid) else {
            return;
        };
        let pal = &cx.palette;
        render::draw_wall_outline(
            painter,
            cam,
            cx,
            wall,
            egui::Stroke::new(3.0_f32, pal.hover),
        );
        let (_, mut ghost) = cx.opening_template(self.kind, wall.kind);
        ghost.wall_id = wid;
        let half = ghost.width * 0.5;
        let len = wall.length();
        if len < ghost.width + 4.0 {
            return;
        }
        ghost.center_offset = center.clamp(half + 2.0, len - half - 2.0);
        render::draw_opening(painter, cam, wall, &ghost, pal, true);
        if cx.view_flags.contains(&ViewFlag::TemporaryDimensions) {
            let dims = TempDims {
                dims: jamb_dims(wall, &ghost, ObjectRef::Wall(wid)).to_vec(),
                editing: None,
            };
            tempdim::draw(&dims, painter, cam, pal, &cx.dim_format());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::WallKind;

    fn setup() -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            7.625,
            109.125,
            WallKind::Exterior,
        );
        (cx, w)
    }

    fn click(t: &mut OpeningTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        t.pointer_down(cx, p.with_down(true))
    }

    #[test]
    fn doors_and_windows_come_from_the_templates() {
        let (mut cx, w) = setup();
        let mut t = OpeningTool::default();
        click(&mut t, &mut cx, 120.4, 0.0);
        let o = cx.floor().openings_on(w).next().unwrap();
        // Exterior wall: exterior door defaults, centered and snapped to 1".
        assert_eq!((o.width, o.height), (36.0, 96.0));
        assert_eq!(o.center_offset, 120.0);

        t.set_variant(ToolId::Window);
        let r = click(&mut t, &mut cx, 40.0, 0.0);
        assert_eq!(r.commit.as_deref(), Some("Place Window"));
        let win = cx
            .floor()
            .openings
            .iter()
            .find(|o| o.kind == OpeningKind::Window)
            .unwrap();
        assert_eq!((win.width, win.height, win.sill_height), (32.0, 72.0, 24.0));
        assert!(cx.extras.openings.contains_key(&win.id));
    }

    #[test]
    fn interior_walls_get_the_interior_door() {
        let (mut cx, _) = setup();
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 60.0),
            Point::new(240.0, 60.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        let mut t = OpeningTool::default();
        click(&mut t, &mut cx, 100.0, 60.0);
        let o = cx.floor().openings_on(w).next().unwrap();
        assert_eq!((o.width, o.height), (30.0, 96.0));
    }

    #[test]
    fn refused_placement_leaves_no_undo_step() {
        let (mut cx, _) = setup();
        let mut t = OpeningTool::default();
        click(&mut t, &mut cx, 120.0, 0.0);
        let n = cx.floor().openings.len();
        click(&mut t, &mut cx, 130.0, 0.0);
        assert_eq!(cx.floor().openings.len(), n);
        assert!(cx.status.contains("does not fit"));
        assert_eq!(cx.undo().as_deref(), Some("Place Door"));
        assert!(cx.floor().openings.is_empty());
        assert!(!cx.can_undo());
    }
}
