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
pub mod details;
pub mod dimension;
pub mod electrical;
pub mod foundation;
pub mod framing;
pub mod library;
pub mod opening;
pub mod pan;
pub mod roof;
pub mod schedule;
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
    Wall {
        kind: WallKind,
    },
    /// A flyout wall: foundation, pony, glass, half-wall, room divider,
    /// railing, deck railing or edge, fencing, or a curved wall.
    WallVariant(wall::WallVariant),
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
    /// A flavor of the electrical tool (the flyout entry picked).
    ElectricalVariant(electrical::ElecVariant),
    /// A flavor of the terrain tool (the flyout entry picked).
    TerrainVariant(terrain::TerrainVariant),
    /// A stair tool of the Stairs flyout.
    StairsVariant(crate::editor::stairs_view::StairKind),
    /// A mode of the roof tool (Roof Plane, Build Roof, Gable Line, ...).
    RoofVariant(roof::RoofMode),
    /// A cabinet kind (Base, Wall, Full Height, ..., Custom Counter Hole).
    CabinetVariant(plan_cabinets::CabinetKind),
    /// A dimension tool (Manual, End to End, ...).
    DimensionVariant(dimension::DimMode),
    /// A text tool (Text, Leader Line, Callout, ...).
    TextVariant(text::TextMode),
    /// A CAD drawing tool (Draw Line, Circle, ...).
    CadVariant(cad::CadMode),
    /// A camera tool (Full Camera, the overviews, the section cameras).
    CameraVariant(camera::CameraVariant),
    /// The slab and foundation tools (Slab flyout, platform holes).
    Foundation,
    /// A flavor of the foundation tool (the flyout entry picked).
    FoundationVariant(foundation::FoundationVariant),
    /// The Trim, Material Region, Deck and 3D Solid tools.
    Details,
    /// A flavor of the details tool (the flyout entry picked).
    DetailsVariant(details::DetailsVariant),
    /// The manual framing tools (General, Floor/Ceiling and Roof Framing flyouts).
    Framing,
    /// A flavor of the framing tool (the flyout entry picked).
    FramingVariant(framing::FramingVariant),
    /// The Schedule tool (Schedule flyout), with the kind it places.
    Schedule,
    ScheduleVariant(plan_core::schedules::ScheduleKind),
    /// Tools > Project Information (opens the dialog, returns to Select).
    ProjectInfo,
}

impl ToolId {
    /// Do both ids belong to the same tool object?
    pub fn same_tool(self, other: ToolId) -> bool {
        match (self.base(), other.base()) {
            (
                ToolId::Wall { .. } | ToolId::WallVariant(_),
                ToolId::Wall { .. } | ToolId::WallVariant(_),
            ) => true,
            (ToolId::Door | ToolId::Window, ToolId::Door | ToolId::Window) => true,
            (
                ToolId::Electrical | ToolId::ElectricalVariant(_),
                ToolId::Electrical | ToolId::ElectricalVariant(_),
            ) => true,
            (
                ToolId::Terrain | ToolId::TerrainVariant(_),
                ToolId::Terrain | ToolId::TerrainVariant(_),
            ) => true,
            (a, b) => a == b,
        }
    }

    /// The plain id of the tool a variant id belongs to (`StairsVariant(_)`
    /// is `Stairs`); other ids are their own base.
    pub fn base(self) -> ToolId {
        match self {
            ToolId::StairsVariant(_) => ToolId::Stairs,
            ToolId::RoofVariant(_) => ToolId::Roof,
            ToolId::CabinetVariant(_) => ToolId::Cabinet,
            ToolId::DimensionVariant(_) => ToolId::Dimension,
            ToolId::TextVariant(_) => ToolId::Text,
            ToolId::CadVariant(_) => ToolId::Cad,
            ToolId::CameraVariant(_) => ToolId::Camera,
            ToolId::FoundationVariant(_) => ToolId::Foundation,
            ToolId::DetailsVariant(_) => ToolId::Details,
            ToolId::FramingVariant(_) => ToolId::Framing,
            ToolId::ScheduleVariant(_) | ToolId::ProjectInfo => ToolId::Schedule,
            other => other,
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
    /// A variant of this tool was picked (wall flavor, door or window, a
    /// `*Variant(..)` payload). Called before `activate` and when switching
    /// between variants.
    fn set_variant(&mut self, _id: ToolId) {}
    /// Called once per frame by the shell, before the canvas is drawn, so a
    /// tool that owns dialogs or palettes can apply what they asked for
    /// without waiting for the next pointer or key event.
    fn frame(&mut self, _cx: &mut EditorContext, _ctx: &egui::Context) {}
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
        Box::new(dimension::DimensionTool::default()),
        Box::new(text::TextTool::default()),
        Box::new(cad::CadTool::default()),
        Box::new(cabinet::CabinetTool::default()),
        Box::new(stairs::StairsTool::default()),
        Box::new(roof::RoofTool::default()),
        Box::new(electrical::ElectricalTool::default()),
        Box::new(library::LibraryTool::default()),
        Box::new(camera::CameraTool::default()),
        Box::new(terrain::TerrainTool::default()),
        Box::new(foundation::FoundationTool::default()),
        Box::new(details::DetailsTool::default()),
        Box::new(framing::FramingTool::default()),
        Box::new(schedule::ScheduleTool::default()),
    ]
}

/// The registry plus the active tool.
pub struct ToolSet {
    tools: Vec<Box<dyn Tool>>,
    active: usize,
    /// The id the active tool was last picked with (carries its variant).
    picked: ToolId,
}

impl ToolSet {
    pub fn new() -> Self {
        Self {
            tools: registry(),
            active: 0,
            picked: ToolId::Select,
        }
    }

    /// The active tool's id; for tools with variants the id it was picked
    /// with (so the toolbar can mark the flyout entry).
    pub fn active_id(&self) -> ToolId {
        let id = self.tools[self.active].id();
        if self.picked != self.picked.base() && self.picked.base() == id {
            self.picked
        } else {
            id
        }
    }

    /// Runs the active tool's per-frame hook.
    pub fn frame(&mut self, cx: &mut EditorContext, ctx: &egui::Context) {
        self.tools[self.active].frame(cx, ctx);
    }

    /// Every registered tool (smoke tests).
    pub fn all_mut(&mut self) -> &mut [Box<dyn Tool>] {
        &mut self.tools
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
        self.picked = id;
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
            ToolId::Foundation,
            ToolId::Details,
            ToolId::Framing,
            ToolId::Schedule,
        ] {
            assert_eq!(ids.iter().filter(|i| i.same_tool(id)).count(), 1, "{id:?}");
        }
    }

    /// Every registered tool can be activated and describes itself; no tool
    /// is a stub any more.
    #[test]
    fn every_tool_activates_with_a_name_and_hint() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let mut set = ToolSet::new();
        let n = set.tools.len();
        for i in 0..n {
            let id = set.tools[i].id();
            set.set_active(&mut cx, id);
            let t = set.active();
            assert!(!t.name().is_empty(), "{id:?} has no name");
            assert!(!t.hint().is_empty(), "{id:?} has no hint");
            assert_ne!(cx.status, "Tool not implemented yet", "{id:?} is a stub");
        }
    }

    #[test]
    fn variant_ids_select_the_exact_sub_tool() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let mut set = ToolSet::new();
        let v = ToolId::StairsVariant(crate::editor::stairs_view::StairKind::Landing);
        set.set_active(&mut cx, v);
        assert_eq!(set.active().name(), "Landing");
        assert_eq!(set.active_id(), v);
        set.set_active(&mut cx, ToolId::CadVariant(cad::CadMode::CircleAboutCenter));
        assert_eq!(set.active().name(), "Circle About Center");
        set.set_active(&mut cx, ToolId::TextVariant(text::TextMode::Callout));
        assert_eq!(set.active().name(), "Callout");
        set.set_active(
            &mut cx,
            ToolId::DimensionVariant(dimension::DimMode::TapeMeasure),
        );
        assert_eq!(set.active().name(), "Tape Measure");
        set.set_active(&mut cx, ToolId::RoofVariant(roof::RoofMode::Hole));
        assert_eq!(set.active_id(), ToolId::RoofVariant(roof::RoofMode::Hole));
        set.set_active(
            &mut cx,
            ToolId::CabinetVariant(plan_cabinets::CabinetKind::Shelf),
        );
        assert_eq!(set.active().name(), "Shelf");
        set.set_active(
            &mut cx,
            ToolId::CameraVariant(camera::CameraVariant::DollHouse),
        );
        assert_eq!(set.active().name(), "Doll House View");
    }
}
