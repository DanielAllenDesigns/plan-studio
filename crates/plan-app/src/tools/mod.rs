//! The tool-plugin architecture (`docs/architecture-tools.md`): every Chief
//! tool is one module here behind the [`Tool`] trait. The application shell
//! forwards pointer and key events to the active tool and draws its overlay.
//!
//! To add a tool: implement [`Tool`] in `tools/<name>.rs`, add a `pub mod`
//! line and one line in [`registry`].

// This is the shared API for tool builders; not every item has a caller yet.
#![allow(dead_code)]

use crate::editor::{Camera, EditAction, EditorContext, SnapResult};
use eframe::egui::{self, Key, Modifiers, PointerButton, Pos2, Vec2};
use plan_core::geometry::Point;
use plan_core::WallKind;

pub mod cabinet;
pub mod cad;
pub mod camera;
pub mod dimension;
pub mod electrical;
pub mod library;
pub mod opening;
pub mod pan;
pub mod roof;
pub mod select;
pub mod stairs;
pub mod terrain;
pub mod text;
pub mod wall;

/// Identifies a tool. Wall flavors share one tool object (so a chain survives
/// switching between them); doors and windows likewise.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolId {
    Select,
    Wall { kind: WallKind },
    Door,
    Window,
    Pan,
    Dimension,
    Text,
    Cad,
    Cabinet,
    Stairs,
    Roof,
    Electrical,
    Library,
    Camera,
    Terrain,
}

impl ToolId {
    /// Do both ids belong to the same tool object?
    pub fn same_tool(self, other: ToolId) -> bool {
        match (self, other) {
            (ToolId::Wall { .. }, ToolId::Wall { .. }) => true,
            (ToolId::Door | ToolId::Window, ToolId::Door | ToolId::Window) => true,
            (a, b) => a == b,
        }
    }
}

/// A pointer event on the canvas, already mapped to world inches.
#[derive(Clone, Copy, Debug)]
pub struct PointerEvent {
    pub world: Point,
    /// `world` after the snap engine without a pending start (grid, object
    /// snaps). Tools with an origin (walls) call `cx.snap_at` themselves.
    pub snapped: Point,
    pub snap: SnapResult,
    pub screen: Pos2,
    pub modifiers: Modifiers,
    pub button: PointerButton,
    /// The primary button is held (during moves).
    pub down: bool,
    /// Pointer movement since the last event, in screen pixels.
    pub drag_delta: Vec2,
}

impl PointerEvent {
    /// An event at `world` as the shell would build it (the snap runs on
    /// `cx`). Screen position uses `cx.px_per_in` with the origin at (0, 0);
    /// handy for driving tools in tests.
    pub fn at(cx: &EditorContext, world: Point) -> Self {
        let snap = cx.snap_at(world, None, false, &[]);
        let s = cx.px_per_in as f32;
        Self {
            world,
            snapped: snap.point,
            snap,
            screen: Pos2::new(world.x as f32 * s, -(world.y as f32) * s),
            modifiers: Modifiers::NONE,
            button: PointerButton::Primary,
            down: false,
            drag_delta: Vec2::ZERO,
        }
    }

    pub fn with_down(mut self, down: bool) -> Self {
        self.down = down;
        self
    }

    pub fn with_modifiers(mut self, m: Modifiers) -> Self {
        self.modifiers = m;
        self
    }
}

/// A key press or typed text, forwarded to the active tool while no text
/// field or dialog has focus.
#[derive(Clone, Debug)]
pub struct KeyEvent {
    pub key: Option<Key>,
    /// Typed characters (digits, quotes) for temporary-dimension entry.
    pub text: Option<String>,
    pub modifiers: Modifiers,
}

impl KeyEvent {
    pub fn key(k: Key) -> Self {
        Self {
            key: Some(k),
            text: None,
            modifiers: Modifiers::NONE,
        }
    }

    pub fn text(s: &str) -> Self {
        Self {
            key: None,
            text: Some(s.to_string()),
            modifiers: Modifiers::NONE,
        }
    }

    pub fn escape() -> Self {
        Self::key(Key::Escape)
    }

    pub fn is(&self, k: Key) -> bool {
        self.key == Some(k)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ToolResult {
    /// The tool used the event (otherwise the shell applies its default,
    /// e.g. Esc returns to Select Objects).
    pub consumed: bool,
    pub repaint: bool,
    pub switch_to: Option<ToolId>,
    /// The undo label of a model change the tool just made.
    pub commit: Option<String>,
}

impl ToolResult {
    pub fn ignored() -> Self {
        Self::default()
    }

    pub fn consumed() -> Self {
        Self {
            consumed: true,
            repaint: true,
            ..Self::default()
        }
    }

    pub fn committed(label: &str) -> Self {
        Self {
            commit: Some(label.to_string()),
            ..Self::consumed()
        }
    }
}

pub trait Tool {
    fn id(&self) -> ToolId;
    /// Chief's name, e.g. "Straight Exterior Wall".
    fn name(&self) -> &'static str;
    /// Status-bar hint.
    fn hint(&self) -> String {
        self.name().to_string()
    }
    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Default
    }
    /// A variant of this tool was picked (wall flavor, door or window). Called
    /// before `activate` and when switching between variants.
    fn set_variant(&mut self, _id: ToolId) {}
    fn activate(&mut self, _cx: &mut EditorContext) {}
    fn deactivate(&mut self, _cx: &mut EditorContext) {}
    fn pointer_down(&mut self, _cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        ToolResult::ignored()
    }
    fn pointer_move(&mut self, _cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        ToolResult::ignored()
    }
    fn pointer_up(&mut self, _cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        ToolResult::ignored()
    }
    fn double_click(&mut self, _cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        ToolResult::ignored()
    }
    /// Esc, Tab, Enter, Delete, arrows and typed text.
    fn key(&mut self, _cx: &mut EditorContext, _k: KeyEvent) -> ToolResult {
        ToolResult::ignored()
    }
    fn draw_overlay(&self, _cx: &EditorContext, _painter: &egui::Painter, _cam: &Camera) {}
    /// Chief's context Edit toolbar for the current selection.
    fn edit_toolbar(&self, _cx: &EditorContext) -> Vec<EditAction> {
        Vec::new()
    }
}

/// Every tool, one line each.
pub fn registry() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(select::SelectTool::default()),
        Box::new(wall::WallTool::default()),
        Box::new(opening::OpeningTool::default()),
        Box::new(pan::PanTool),
        Box::new(dimension::DimensionTool),
        Box::new(text::TextTool),
        Box::new(cad::CadTool),
        Box::new(cabinet::CabinetTool),
        Box::new(stairs::StairsTool),
        Box::new(roof::RoofTool),
        Box::new(electrical::ElectricalTool),
        Box::new(library::LibraryTool),
        Box::new(camera::CameraTool),
        Box::new(terrain::TerrainTool),
    ]
}

/// The registry plus the active tool.
pub struct ToolSet {
    tools: Vec<Box<dyn Tool>>,
    active: usize,
}

impl ToolSet {
    pub fn new() -> Self {
        Self {
            tools: registry(),
            active: 0,
        }
    }

    pub fn active_id(&self) -> ToolId {
        self.tools[self.active].id()
    }

    pub fn active(&self) -> &dyn Tool {
        self.tools[self.active].as_ref()
    }

    pub fn active_mut(&mut self) -> &mut dyn Tool {
        self.tools[self.active].as_mut()
    }

    /// Makes `id` the active tool. Switching between variants of one tool
    /// (wall flavors) keeps its state.
    pub fn set_active(&mut self, cx: &mut EditorContext, id: ToolId) {
        let Some(next) = self.tools.iter().position(|t| t.id().same_tool(id)) else {
            return;
        };
        if next == self.active {
            self.tools[next].set_variant(id);
            return;
        }
        self.tools[self.active].deactivate(cx);
        self.active = next;
        self.tools[next].set_variant(id);
        cx.hover = None;
        cx.readout = None;
        cx.last_snap = None;
        self.tools[next].activate(cx);
    }

    /// Resets the active tool's state (new file, floor change).
    pub fn restart(&mut self, cx: &mut EditorContext) {
        self.tools[self.active].deactivate(cx);
        self.tools[self.active].activate(cx);
    }
}

impl Default for ToolSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_every_tool_id_once() {
        let ids: Vec<ToolId> = registry().iter().map(|t| t.id()).collect();
        for id in [
            ToolId::Select,
            ToolId::Wall {
                kind: WallKind::Exterior,
            },
            ToolId::Door,
            ToolId::Pan,
            ToolId::Dimension,
            ToolId::Text,
            ToolId::Cad,
            ToolId::Cabinet,
            ToolId::Stairs,
            ToolId::Roof,
            ToolId::Electrical,
            ToolId::Library,
            ToolId::Camera,
            ToolId::Terrain,
        ] {
            assert_eq!(ids.iter().filter(|i| i.same_tool(id)).count(), 1, "{id:?}");
        }
    }

    #[test]
    fn stubs_say_they_are_not_implemented() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let mut set = ToolSet::new();
        set.set_active(&mut cx, ToolId::Stairs);
        assert_eq!(set.active_id(), ToolId::Stairs);
        assert_eq!(cx.status, "Tool not implemented yet");
    }
}
