//! Text tool: not built yet. Fill in the [`Tool`] methods (see
//! `docs/architecture-tools.md` and the matching `docs/parity/*.md`).

use super::{Tool, ToolId};
use crate::editor::EditorContext;

pub struct TextTool;

impl Tool for TextTool {
    fn id(&self) -> ToolId {
        ToolId::Text
    }

    fn name(&self) -> &'static str {
        "Text"
    }

    fn hint(&self) -> String {
        "Text: not yet implemented".into()
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = "Tool not implemented yet".into();
    }
}
