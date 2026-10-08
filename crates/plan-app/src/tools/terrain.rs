//! Terrain tool: not built yet. Fill in the [`Tool`] methods (see
//! `docs/architecture-tools.md` and the matching `docs/parity/*.md`).

use super::{Tool, ToolId};
use crate::editor::EditorContext;

pub struct TerrainTool;

impl Tool for TerrainTool {
    fn id(&self) -> ToolId {
        ToolId::Terrain
    }

    fn name(&self) -> &'static str {
        "Terrain"
    }

    fn hint(&self) -> String {
        "Terrain: not yet implemented".into()
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        cx.status = "Tool not implemented yet".into();
    }
}
