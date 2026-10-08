//! Library Symbol tool: not built yet. Fill in the [`Tool`] methods (see
//! `docs/architecture-tools.md` and the matching `docs/parity/*.md`).

use super::{Tool, ToolId};
use crate::editor::EditorContext;

pub struct LibraryTool;

impl Tool for LibraryTool {
    fn id(&self) -> ToolId {
        ToolId::Library
    }

    fn name(&self) -> &'static str {
        "Library Symbol"
    }

    fn hint(&self) -> String {
        "Library Symbol: not yet implemented".into()
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = "Tool not implemented yet".into();
    }
}
