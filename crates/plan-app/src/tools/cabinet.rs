//! Cabinet tool: not built yet. Fill in the [`Tool`] methods (see
//! `docs/architecture-tools.md` and the matching `docs/parity/*.md`).

use super::{Tool, ToolId};
use crate::editor::EditorContext;

pub struct CabinetTool;

impl Tool for CabinetTool {
    fn id(&self) -> ToolId {
        ToolId::Cabinet
    }

    fn name(&self) -> &'static str {
        "Cabinet"
    }

    fn hint(&self) -> String {
        "Cabinet: not yet implemented".into()
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = "Tool not implemented yet".into();
    }
}
