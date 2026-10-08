//! Dimension tool: not built yet. Fill in the [`Tool`] methods (see
//! `docs/architecture-tools.md` and the matching `docs/parity/*.md`).

use super::{Tool, ToolId};
use crate::editor::EditorContext;

pub struct DimensionTool;

impl Tool for DimensionTool {
    fn id(&self) -> ToolId {
        ToolId::Dimension
    }

    fn name(&self) -> &'static str {
        "Dimension"
    }

    fn hint(&self) -> String {
        "Dimension: not yet implemented".into()
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = "Tool not implemented yet".into();
    }
}
