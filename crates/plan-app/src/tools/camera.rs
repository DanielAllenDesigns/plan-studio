//! Camera tool: not built yet. Fill in the [`Tool`] methods (see
//! `docs/architecture-tools.md` and the matching `docs/parity/*.md`).

use super::{Tool, ToolId};
use crate::editor::EditorContext;

pub struct CameraTool;

impl Tool for CameraTool {
    fn id(&self) -> ToolId {
        ToolId::Camera
    }

    fn name(&self) -> &'static str {
        "Camera"
    }

    fn hint(&self) -> String {
        "Camera: not yet implemented".into()
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = "Tool not implemented yet".into();
    }
}
