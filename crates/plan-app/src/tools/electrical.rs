//! Electrical tool: not built yet. Fill in the [`Tool`] methods (see
//! `docs/architecture-tools.md` and the matching `docs/parity/*.md`).

use super::{Tool, ToolId};
use crate::editor::EditorContext;

pub struct ElectricalTool;

impl Tool for ElectricalTool {
    fn id(&self) -> ToolId {
        ToolId::Electrical
    }

    fn name(&self) -> &'static str {
        "Electrical"
    }

    fn hint(&self) -> String {
        "Electrical: not yet implemented".into()
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = "Tool not implemented yet".into();
    }
}
