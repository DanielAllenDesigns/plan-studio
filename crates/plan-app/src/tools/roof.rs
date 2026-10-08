//! Roof tool: not built yet. Fill in the [`Tool`] methods (see
//! `docs/architecture-tools.md` and the matching `docs/parity/*.md`).

use super::{Tool, ToolId};
use crate::editor::EditorContext;

pub struct RoofTool;

impl Tool for RoofTool {
    fn id(&self) -> ToolId {
        ToolId::Roof
    }

    fn name(&self) -> &'static str {
        "Roof"
    }

    fn hint(&self) -> String {
        "Roof: not yet implemented".into()
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = "Tool not implemented yet".into();
    }
}
