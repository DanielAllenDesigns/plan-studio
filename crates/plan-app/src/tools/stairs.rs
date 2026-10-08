//! Stairs tool: not built yet. Fill in the [`Tool`] methods (see
//! `docs/architecture-tools.md` and the matching `docs/parity/*.md`).

use super::{Tool, ToolId};
use crate::editor::EditorContext;

pub struct StairsTool;

impl Tool for StairsTool {
    fn id(&self) -> ToolId {
        ToolId::Stairs
    }

    fn name(&self) -> &'static str {
        "Stairs"
    }

    fn hint(&self) -> String {
        "Stairs: not yet implemented".into()
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = "Tool not implemented yet".into();
    }
}
