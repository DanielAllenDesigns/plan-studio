//! CAD tool: not built yet. Fill in the [`Tool`] methods (see
//! `docs/architecture-tools.md` and the matching `docs/parity/*.md`).

use super::{Tool, ToolId};
use crate::editor::EditorContext;

pub struct CadTool;

impl Tool for CadTool {
    fn id(&self) -> ToolId {
        ToolId::Cad
    }

    fn name(&self) -> &'static str {
        "CAD"
    }

    fn hint(&self) -> String {
        "CAD: not yet implemented".into()
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = "Tool not implemented yet".into();
    }
}
