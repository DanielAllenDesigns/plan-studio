//! Pan Window (H): dragging with the primary button moves the view.

use super::{PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::{EditorContext, EditorRequest};
use eframe::egui;

pub struct PanTool;

impl Tool for PanTool {
    fn id(&self) -> ToolId {
        ToolId::Pan
    }

    fn name(&self) -> &'static str {
        "Pan Window"
    }

    fn hint(&self) -> String {
        "Pan: drag to move the view; Esc or Select returns".into()
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Grab
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if p.down && p.drag_delta != egui::Vec2::ZERO {
            cx.requests.push(EditorRequest::PanPixels(p.drag_delta));
            return ToolResult::consumed();
        }
        ToolResult::ignored()
    }
}
