//! Chief Architect-style toolbars: data tables plus a small renderer.
//!
//! The three bars mirror `docs/chief-x18-toolbars.md` (row 1, row 2 and the
//! right-edge view bar). Every flyout's entries come from
//! `docs/chief-x18-subtools.md` and are built by one function per group
//! (e.g. [`straight_wall`]); the toolbar, the Build/Terrain/CAD menus and the
//! hotkey tooltips all read those same tables. Rendering returns [`Action`]s;
//! the app applies them.

pub mod config;

use crate::icons;
use crate::shell::view3d_panel::View3dCommand;
use crate::theme::{scale, CanvasTheme};
use crate::tools::camera::CameraVariant;
use crate::tools::details::DetailsVariant;
use crate::tools::foundation::FoundationVariant;
use crate::tools::framing::FramingVariant;
use crate::tools::images::ImageMode;
use crate::tools::opening::OpeningVariant;
use crate::tools::painters::PainterMode;
use crate::tools::wall::{WallStyle as Style, WallVariant};
use crate::tools::ToolId;

use eframe::egui::{
    self, Align, Color32, Image, Key, Layout, Modifiers, PopupCloseBehavior, Rect, Sense, Shape,
    Stroke, Vec2,
};
use plan_core::WallKind;
use std::collections::HashSet;
use std::time::{Duration, Instant};

const BUTTON_PX: f32 = 28.0;
const ICON_PX: f32 = 20.0;
const ARROW_PX: f32 = 12.0;
/// Gap between neighbours; 28 + 4 gives the 32 px pitch.
const GAP_PX: f32 = 4.0;
const VIEW_SELECTOR_PX: f32 = 270.0;
const FLOOR_LABEL_PX: f32 = 20.0;
/// Flyout popups: row height (bigger than a menu row) and minimum width.
const FLYOUT_ROW_PX: f32 = 30.0;
const FLYOUT_MIN_WIDTH: f32 = 280.0;
/// The plate behind toolbar icons when Preferences > Icon halo is on.
const HALO_FILL: Color32 = Color32::from_rgb(0x41, 0x41, 0x41);
const SEPARATOR_COLOR: Color32 = Color32::from_rgb(0x70, 0x70, 0x70);
/// How long a multi-key hotkey prefix (like the `D` of `D, H`) stays pending.
pub const SEQUENCE_TIMEOUT: Duration = Duration::from_millis(1500);

/// Independent view toggles stored on the app.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ViewFlag {
    ReferenceDisplay,
    ReferenceGrid,
    Crosshairs,
    Color,
    LineWeights,
    DrawingSheet,
    PrintPreview,
    TemporaryDimensions,
    ConnectCad,
    ArcCenters,
    SunAngle,
}

/// Docked side panels opened from the view bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dock {
    Library,
    Project,
    LayerDisplay,
    /// The Plan Agent ("Ask to make changes"); see `shell::agent_panel`.
    Agent,
}

impl Dock {
    pub fn title(self) -> &'static str {
        match self {
            Dock::Library => "Library Browser",
            Dock::Project => "Project Browser",
            Dock::LayerDisplay => "Active Layer Display Options",
            Dock::Agent => "Plan Agent",
        }
    }
}

/// Terrain menu commands that are not tools.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerrainCommand {
    Specification,
    Clear,
    HoleAroundBuilding,
}

/// File > Export / Import and CAD > CAD to Walls.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileCommand {
    /// File > Export > DXF...: the active floor.
    ExportDxf,
    /// File > Export > Elevation DXF...: Front, Back, Left and Right.
    ExportElevationsDxf,
    /// File > Import > Import Drawing (DXF)...
    ImportDxf,
    /// CAD > CAD to Walls...
    CadToWalls,
    /// File > Import > Layout (JSON)...: a saved layout replaces the plan's.
    ImportLayout,
    /// File > Export > Layout (JSON)...
    ExportLayout,
}

/// Build > Framing commands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FramingCommand {
    /// Frame the active floor (walls, floor platforms, roof).
    Build,
    /// Frame every floor.
    BuildAll,
    /// Delete the active floor's framing.
    Delete,
    /// Tools > Schedules > Framing Takeoff...
    Takeoff,
}

/// Everything the UI can ask the app to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    SetTool(ToolId),
    /// The wall flyout's currently selected variant (hotkey alias `2`).
    CurrentWall,
    FileNew,
    /// File > Import > Chief Plan... (walls, openings, floors, rooms from a .plan).
    ImportChiefPlan,
    /// File > New Layout: a layout from Daniel's layout template.
    FileNewLayout,
    FileOpen,
    FileSave,
    FileSaveAs,
    /// File > Templates > Save Current Defaults as My Template...
    SaveTemplate,
    /// File > Templates > Reset to Chief X18 Template
    ResetTemplate,
    /// File > Templates > Import Chief Template...
    ImportChiefTemplate,

    Quit,
    ZoomIn,
    ZoomOut,
    UndoZoom,
    /// Edit > Undo (the label lives in `BarState`).
    Undo,
    Redo,
    FillWindow,
    FloorUp,
    FloorDown,
    TogglePan,
    ToggleFlag(ViewFlag),
    ToggleDock(Dock),
    SetTheme(CanvasTheme),
    ShowAbout,
    /// Edit > Default Settings...
    DefaultSettings,
    /// Tools > Project Information... (`build_tools::open_project_info`).
    ProjectInfo,
    /// Edit > Find/Replace Text...
    FindReplaceText,
    /// Edit > Snap Settings...
    SnapSettings,
    /// Edit > Edit Behaviors...
    EditBehaviors,
    /// Tools > Toolbars and Hotkeys > Customize Hotkeys...
    OpenHotkeyDialog,
    /// Tools > Layer Settings > Display Options...
    OpenLayerDisplay,
    // Build > Floor, Tools > Space Planning / Checks / Schedules (build_tools).
    BuildNewFloor,
    InsertFloor,
    /// Build > Floor > Insert New Floor Below.
    InsertFloorBelow,
    /// Floor Defaults of the active floor (toolbar button, Build > Floor).
    FloorDefaults,
    /// Edit > Default Settings > Floors and Rooms > Floor Defaults.
    PlanFloorDefaults,
    /// The Reference Display dialog (Tools > Floor/Reference Display).
    ReferenceDisplayOptions,
    DeleteFloor,
    DeleteFoundation,
    ExchangeFloorAbove,
    ExchangeFloorBelow,
    BuildFoundation,
    RebuildAll,
    SpacePlanning,
    PlanCheck,
    DoorWindowCheck,
    PlanFootprint,
    MaterialsList,
    DoorSchedule,
    WindowSchedule,
    RoomSchedule,
    WallSchedule,
    CreateConstructionSet,
    NotImplemented(&'static str),
    /// A command of an object type (`EditorContext::run_custom`), such as
    /// Delete Roof Planes.
    Custom(&'static str),
    /// Terrain menu commands.
    Terrain(TerrainCommand),
    /// File exchange and CAD to Walls (`dialogs::exchange`).
    File(FileCommand),
    /// Framing (`dialogs::exchange`).
    Framing(FramingCommand),
    /// Row-1 view selector: activate the saved plan view with this index.
    PlanView(usize),

    /// The 3D view, cameras and rendering (`shell::view3d_panel`).
    View3d(View3dCommand),
    /// The layout view, Send to Layout and Print (`shell::layout_window`).
    Layout(crate::shell::layout_window::LayoutCommand),
}

/// One toolbar button (or one flyout variant).
#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub icon: &'static str,
    pub name: &'static str,
    pub hotkey: Option<&'static str>,
    pub action: Action,
    pub enabled: bool,
    /// Draw a separator above this entry in the flyout list and in menus.
    pub sep_before: bool,
}

/// A button with a variant list; the face shows `entries[current]`.
#[derive(Clone, Debug)]
pub struct Flyout {
    /// The submenu title used when this flyout becomes a menu.
    pub group: &'static str,
    pub entries: Vec<Item>,
    pub current: usize,
}

/// One slot on a bar.
#[derive(Clone, Debug)]
pub enum Slot {
    Separator,
    Button(Item),
    Toggle(Item),
    Flyout(Flyout),
    /// The saved-view drop-down (row 1).
    ViewSelector,
    /// The current floor number (row 1).
    FloorLabel,
}

/// App state the bars and menus need to draw themselves.
pub struct BarState<'a> {
    pub tool: ToolId,
    pub flags: &'a HashSet<ViewFlag>,
    pub dock: Option<Dock>,
    pub floor: usize,
    pub floor_count: usize,
    pub view_name: &'a str,
    /// The saved plan views the row-1 selector lists.
    pub views: &'a [plan_core::SavedPlanView],
    /// Global UI brightness (0.6..=1.0), applied to icon tints and fills.
    pub brightness: f32,
    /// Label of the step Undo would revert ("Move Wall"); `None` when there is none.
    pub undo_label: Option<&'a str>,
    pub redo_label: Option<&'a str>,
    /// The live hotkey map, so menu rows show the keys that really trigger
    /// them (Daniel's customized ones, then the user's edits).
    pub hotkeys: Option<&'a crate::shell::hotkeys::HotkeyMap>,
}

impl BarState<'_> {
    /// The hotkey text for the command `name`: the live map's keys, or
    /// `fallback` when no map is attached.
    pub fn hotkey(&self, name: &str, fallback: &str) -> String {
        match self.hotkeys {
            Some(map) => map.hotkey_text(name),
            None => fallback.to_string(),
        }
    }
}

/// The three bars, with their flyout selections.
pub struct Toolbars {
    pub row1: Vec<Slot>,
    pub row2: Vec<Slot>,
    pub view: Vec<Slot>,
}

impl Toolbars {
    pub fn new() -> Self {
        Self {
            row1: row1_slots(),
            row2: row2_slots(),
            view: view_slots(),
        }
    }

    /// The action of the wall flyout's current entry (hotkey alias `2`).
    pub fn wall_action(&self) -> Action {
        self.row2
            .iter()
            .find_map(|s| match s {
                Slot::Flyout(f) if f.group == "Straight Wall" => {
                    f.entries.get(f.current).map(|e| e.action)
                }
                _ => None,
            })
            .unwrap_or(EXTERIOR_WALL)
    }
}

// ----- hotkeys -----

const SELECT: Action = Action::SetTool(ToolId::Select);
const EXTERIOR_WALL: Action = Action::SetTool(ToolId::Wall {
    kind: WallKind::Exterior,
});
const INTERIOR_WALL: Action = Action::SetTool(ToolId::Wall {
    kind: WallKind::Interior,
});
const DOOR: Action = Action::SetTool(ToolId::Door);
const DOORWAY: Action = door_action(plan_core::OpeningStyle::Doorway);
const SLIDING_DOOR: Action = door_action(plan_core::OpeningStyle::Sliding);
const POCKET_DOOR: Action = door_action(plan_core::OpeningStyle::Pocket);
const GARAGE_DOOR: Action = door_action(plan_core::OpeningStyle::Garage);

/// The action of a Door flyout entry (a `const` form of
/// `OpeningVariant::door(style).tool_id()` for the binding table).
const fn door_action(style: plan_core::OpeningStyle) -> Action {
    Action::SetTool(ToolId::OpeningVariant(OpeningVariant::door(style)))
}
const WINDOW: Action = Action::SetTool(ToolId::Window);

/// One keyboard binding: a key (or key sequence) plus its modifiers.
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    pub keys: &'static [Key],
    pub shift: bool,
    /// Control key (the physical Control key on macOS).
    pub ctrl: bool,
    /// Command on macOS, Control elsewhere.
    pub command: bool,
    pub action: Action,
}

const fn bind(
    _chief_label: &'static str,
    keys: &'static [Key],
    (shift, ctrl, command): (bool, bool, bool),
    action: Action,
) -> Binding {
    Binding {
        keys,
        shift,
        ctrl,
        command,
        action,
    }
}

const NONE: (bool, bool, bool) = (false, false, false);
const SHIFT: (bool, bool, bool) = (true, false, false);
const CTRL: (bool, bool, bool) = (false, true, false);
const CMD: (bool, bool, bool) = (false, false, true);

/// Every keyboard binding, in one place. Bindings are only active while no
/// text field has keyboard focus.
///
/// Not bound on purpose: four-modifier chords such as Straight Interior Wall
/// (control-option-command-6); they are shown in tooltips and menus only.
pub const BINDINGS: &[Binding] = &[
    // Select Objects.
    bind("Space", &[Key::Space], NONE, SELECT),
    // Straight Exterior Wall and Window (Chief defaults).
    bind("\u{21E7}Q", &[Key::Q], SHIFT, EXTERIOR_WALL),
    bind("\u{21E7}W", &[Key::W], SHIFT, WINDOW),
    // Hinged Door is a two-key sequence.
    bind("D, H", &[Key::D, Key::H], NONE, DOOR),
    // The other door flavors with a two-key Chief hotkey.
    bind("D, W", &[Key::D, Key::W], NONE, DOORWAY),
    bind("S, D", &[Key::S, Key::D], NONE, SLIDING_DOOR),
    bind("D, P", &[Key::D, Key::P], NONE, POCKET_DOOR),
    bind("G, D", &[Key::G, Key::D], NONE, GARAGE_DOOR),
    // Chief hotkeys for the build, CAD and 3D tools.
    bind(
        "\u{21E7}Y",
        &[Key::Y],
        SHIFT,
        Action::SetTool(ToolId::StairsVariant(
            crate::editor::stairs_view::StairKind::Draw,
        )),
    ),
    bind(
        "\u{21E7}T",
        &[Key::T],
        SHIFT,
        Action::SetTool(ToolId::CabinetVariant(plan_cabinets::CabinetKind::Base)),
    ),
    bind(
        "\u{21E7}A",
        &[Key::A],
        SHIFT,
        Action::SetTool(ToolId::DimensionVariant(
            crate::tools::dimension::DimMode::AutoExterior,
        )),
    ),
    bind(
        "Q",
        &[Key::Q],
        NONE,
        Action::SetTool(ToolId::RoofVariant(crate::tools::roof::RoofMode::Plane)),
    ),
    bind(
        "Y",
        &[Key::Y],
        NONE,
        Action::SetTool(ToolId::TextVariant(crate::tools::text::TextMode::Text)),
    ),
    bind(
        "K",
        &[Key::K],
        NONE,
        Action::SetTool(ToolId::CadVariant(crate::tools::cad::CadMode::Circle)),
    ),
    bind(
        "\u{21E7}P",
        &[Key::P],
        SHIFT,
        Action::SetTool(ToolId::CadVariant(crate::tools::cad::CadMode::RectPolyline)),
    ),
    bind(
        "\u{21E7}J",
        &[Key::J],
        SHIFT,
        Action::View3d(View3dCommand::Tool(CameraVariant::FullCamera)),
    ),
    bind(
        "\u{21E7}K",
        &[Key::K],
        SHIFT,
        Action::View3d(View3dCommand::Tool(CameraVariant::FullOverview)),
    ),
    // View and window.
    bind("\u{2303}F", &[Key::F], CTRL, Action::FillWindow),
    bind("H", &[Key::H], NONE, Action::TogglePan),
    bind("F8", &[Key::F8], NONE, Action::ToggleFlag(ViewFlag::Color)),
    bind(
        "\u{21E7}F9",
        &[Key::F9],
        SHIFT,
        Action::ToggleFlag(ViewFlag::ReferenceGrid),
    ),
    bind(
        "\u{2318}L",
        &[Key::L],
        CMD,
        Action::ToggleDock(Dock::Library),
    ),
    // Undo and redo.
    bind("\u{2318}Z", &[Key::Z], CMD, Action::Undo),
    bind(
        "\u{21E7}\u{2318}Z",
        &[Key::Z],
        (true, false, true),
        Action::Redo,
    ),
    bind("\u{2318}Y", &[Key::Y], CMD, Action::Redo),
    // File.
    bind("\u{2318}N", &[Key::N], CMD, Action::FileNew),
    bind("\u{2318}O", &[Key::O], CMD, Action::FileOpen),
    // 3D > 3D View Defaults.
    bind(
        "\u{2318}1",
        &[Key::Num1],
        CMD,
        Action::View3d(View3dCommand::Defaults),
    ),
    bind("\u{2318}S", &[Key::S], CMD, Action::FileSave),
    // Plan Studio's original number-key aliases.
    bind("1", &[Key::Num1], NONE, SELECT),
    bind("2", &[Key::Num2], NONE, Action::CurrentWall),
    bind("3", &[Key::Num3], NONE, DOOR),
    bind("4", &[Key::Num4], NONE, WINDOW),
];

impl Binding {
    fn matches(&self, m: &Modifiers) -> bool {
        if m.alt || m.shift != self.shift {
            return false;
        }
        if self.command {
            m.command
        } else if self.ctrl {
            m.ctrl && !m.mac_cmd
        } else {
            !m.ctrl && !m.command && !m.mac_cmd
        }
    }
}

/// Turns key presses into actions, including two-key sequences like `D, H`.
/// Superseded at runtime by `shell::hotkeys::HotkeyState`; kept for its tests.
#[derive(Default)]
#[allow(dead_code)]
pub struct Hotkeys {
    pending: Vec<Key>,
    since: Option<Instant>,
}

#[allow(dead_code)]
impl Hotkeys {
    /// Reads this frame's key presses (nothing while a text field has focus)
    /// and returns the actions they trigger.
    pub fn poll(&mut self, ctx: &egui::Context) -> Vec<Action> {
        let mut out = Vec::new();
        self.expire(Instant::now());
        if ctx.wants_keyboard_input() {
            self.clear();
            return out;
        }
        let presses: Vec<(Key, Modifiers)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        let now = Instant::now();
        for (key, mods) in presses {
            self.press(key, &mods, now, &mut out);
        }
        if !self.pending.is_empty() {
            ctx.request_repaint_after(SEQUENCE_TIMEOUT);
        }
        out
    }

    /// The pending sequence prefix for the status bar, like `"D, ..."`.
    pub fn pending_label(&self) -> Option<String> {
        if self.pending.is_empty() {
            return None;
        }
        let keys: Vec<&str> = self.pending.iter().map(|k| k.name()).collect();
        Some(format!("{}, ...", keys.join(", ")))
    }

    fn clear(&mut self) {
        self.pending.clear();
        self.since = None;
    }

    fn expire(&mut self, now: Instant) {
        if self
            .since
            .is_some_and(|t| now.saturating_duration_since(t) >= SEQUENCE_TIMEOUT)
        {
            self.clear();
        }
    }

    fn press(&mut self, key: Key, mods: &Modifiers, now: Instant, out: &mut Vec<Action>) {
        if key == Key::Escape {
            self.clear();
            return;
        }
        self.expire(now);
        if !self.pending.is_empty() {
            let mut seq = self.pending.clone();
            seq.push(key);
            if let Some(b) = BINDINGS
                .iter()
                .find(|b| b.keys == seq.as_slice() && b.matches(mods))
            {
                out.push(b.action);
                self.clear();
                return;
            }
            let extends = BINDINGS
                .iter()
                .any(|b| b.keys.len() > seq.len() && b.keys.starts_with(&seq) && b.matches(mods));
            if extends {
                self.pending = seq;
                self.since = Some(now);
                return;
            }
            // Not a continuation: drop the prefix and treat this as a fresh press.
            self.clear();
        }
        if let Some(b) = BINDINGS
            .iter()
            .find(|b| b.keys.first() == Some(&key) && b.matches(mods))
        {
            if b.keys.len() == 1 {
                out.push(b.action);
            } else {
                self.pending = vec![key];
                self.since = Some(now);
            }
        }
    }
}

// ----- table helpers -----

fn item(icon: &'static str, name: &'static str, action: Action) -> Item {
    Item {
        icon,
        name,
        hotkey: None,
        action,
        enabled: true,
        sep_before: false,
    }
}

fn todo(icon: &'static str, name: &'static str) -> Item {
    item(icon, name, Action::NotImplemented(name))
}

fn with_hotkey(mut it: Item, key: &'static str) -> Item {
    it.hotkey = Some(key);
    it
}

/// Marks `it` as starting a new section of its flyout or submenu.
fn sep(mut it: Item) -> Item {
    it.sep_before = true;
    it
}

fn button(icon: &'static str, name: &'static str) -> Slot {
    Slot::Button(todo(icon, name))
}

fn toggle(icon: &'static str, name: &'static str) -> Slot {
    Slot::Toggle(todo(icon, name))
}

fn fly(group: &'static str, entries: Vec<Item>) -> Flyout {
    Flyout {
        group,
        entries,
        current: 0,
    }
}

fn flyout_slot(f: Flyout) -> Slot {
    Slot::Flyout(f)
}

fn flag_toggle(icon: &'static str, name: &'static str, flag: ViewFlag) -> Slot {
    Slot::Toggle(item(icon, name, Action::ToggleFlag(flag)))
}

/// A wall flyout entry that draws `style` (straight or curved).
fn wall_item(icon: &'static str, style: Style, curved: bool) -> Item {
    let v = WallVariant { style, curved };
    item(icon, v.name(), Action::SetTool(ToolId::WallVariant(v)))
}

// ----- flyout tables (docs/chief-x18-subtools.md) -----

/// Walls: Straight Wall Tools, in Build-menu order.
pub fn straight_wall() -> Flyout {
    fly(
        "Straight Wall",
        vec![
            with_hotkey(
                item("wall_exterior", "Straight Exterior Wall", EXTERIOR_WALL),
                "\u{21E7}Q",
            ),
            with_hotkey(
                item("wall_interior", "Straight Interior Wall", INTERIOR_WALL),
                "\u{2303}\u{2325}\u{2318}6",
            ),
            wall_item("wall_foundation", Style::Foundation, false),
            wall_item("wall_pony", Style::Pony, false),
            wall_item("wall_exterior", Style::Glass, false),
            wall_item("wall_pony", Style::GlassPony, false),
            wall_item("wall_half", Style::Half, false),
            wall_item("wall_room_divider", Style::RoomDivider, false),
            det("slab", DetailsVariant::SlabFooting),
            det("wall_hatch", DetailsVariant::WallHatching),
            det("wall_hatch", DetailsVariant::WallMaterialRegion),
        ],
    )
}

pub fn curved_wall() -> Flyout {
    fly(
        "Curved Wall",
        vec![
            wall_item("wall_curved", Style::Exterior, true),
            wall_item("wall_curved_interior", Style::Interior, true),
            wall_item("wall_curved", Style::Foundation, true),
            wall_item("wall_curved", Style::Pony, true),
            wall_item("wall_curved", Style::Half, true),
        ],
    )
}

pub fn railing_deck() -> Flyout {
    fly(
        "Railing and Deck",
        vec![
            // Chief's Command-Q is the macOS Quit shortcut: shown, not bound.
            with_hotkey(wall_item("railing", Style::Railing, false), "\u{2318}Q"),
            wall_item("railing_curved", Style::Railing, true),
            sep(wall_item("deck_railing", Style::DeckRailing, false)),
            wall_item("deck_railing", Style::DeckRailing, true),
            sep(wall_item("deck_edge", Style::DeckEdge, false)),
            wall_item("deck_edge", Style::DeckEdge, true),
            sep(det("deck_edge", DetailsVariant::PolygonDeck)),
        ],
    )
}

pub fn fencing() -> Flyout {
    fly(
        "Fencing",
        vec![
            wall_item("railing", Style::Fencing, false),
            wall_item("railing_curved", Style::Fencing, true),
        ],
    )
}

/// A Door or Window flyout entry that places `v`.
fn opening_item(icon: &'static str, v: OpeningVariant, key: Option<&'static str>) -> Item {
    let it = item(icon, v.name(), Action::SetTool(v.tool_id()));
    match key {
        Some(k) => with_hotkey(it, k),
        None => it,
    }
}

pub fn door() -> Flyout {
    use plan_core::OpeningStyle as S;
    let d = |icon, style, key| opening_item(icon, OpeningVariant::door(style), key);
    fly(
        "Door",
        vec![
            d("door_hinged", S::Hinged, Some("D, H")),
            d("doorway", S::Doorway, Some("D, W")),
            d("door_sliding", S::Sliding, Some("S, D")),
            d("door_pocket", S::Pocket, Some("D, P")),
            d("door_bifold", S::Bifold, Some("\u{2303}\u{2325}\u{2318}O")),
            d("door_barn", S::Barn, Some("\u{2303}\u{2325}\u{2318}P")),
            d("door_hinged", S::Fixed, Some("\u{2303}\u{2325}\u{2318}R")),
            d("door_garage", S::Garage, Some("G, D")),
            d("door_hinged", S::Shower, Some("\u{2303}\u{2325}\u{2318}Q")),
            // Plan Studio's own entry: Chief picks double doors in the dialog.
            sep(d("door_hinged", S::DoubleDoor, None)),
        ],
    )
}

pub fn window() -> Flyout {
    use plan_core::OpeningStyle as S;
    let w = |icon, style, key| opening_item(icon, OpeningVariant::window(style), key);
    fly(
        "Window",
        vec![
            w("window", S::Window, Some("\u{21E7}W")),
            w(
                "window_bay",
                S::BayWindow,
                Some("\u{2303}\u{2325}\u{2318}S"),
            ),
            w(
                "window_bow",
                S::BowWindow,
                Some("\u{2303}\u{2325}\u{2318}T"),
            ),
            w(
                "window_box",
                S::BoxWindow,
                Some("\u{2303}\u{2325}\u{2318}U"),
            ),
            w(
                "pass_through",
                S::PassThrough,
                Some("\u{2303}\u{2325}\u{2318}V"),
            ),
            w(
                "pass_through",
                S::WallNiche,
                Some("\u{2303}\u{2325}\u{2318}W"),
            ),
            // Plan Studio's own entries: Chief picks the window type in the
            // dialog.
            sep(w("window", S::Casement, None)),
            w("window", S::Fixed, None),
            w("window", S::SlidingWindow, None),
            w("window", S::Awning, None),
            w("window", S::Hopper, None),
        ],
    )
}

pub fn cabinet() -> Flyout {
    use plan_cabinets::CabinetKind as K;
    let cab = |icon, name, key: Option<&'static str>, v: K| {
        let it = item(icon, name, Action::SetTool(ToolId::CabinetVariant(v)));
        match key {
            Some(k) => with_hotkey(it, k),
            None => it,
        }
    };
    fly(
        "Cabinet",
        vec![
            cab("cabinet_base", "Base Cabinet", Some("\u{21E7}T"), K::Base),
            cab("cabinet_wall", "Wall Cabinet", Some("\u{2318}T"), K::Wall),
            cab(
                "cabinet_full",
                "Full Height",
                Some("\u{2303}\u{2325}\u{2318}X"),
                K::FullHeight,
            ),
            cab("soffit", "Soffit", Some("T"), K::Soffit),
            cab(
                "shelf",
                "Shelf",
                Some("\u{2303}\u{2325}\u{2318}Y"),
                K::Shelf,
            ),
            cab(
                "partition",
                "Partition",
                Some("\u{2303}\u{2325}\u{2318}Z"),
                K::Partition,
            ),
            cab(
                "cabinet_base",
                "Base Filler",
                Some("\u{2303}\u{2325}\u{2318}0"),
                K::BaseFiller,
            ),
            cab(
                "cabinet_wall",
                "Wall Filler",
                Some("\u{2303}\u{2325}\u{2318}1"),
                K::WallFiller,
            ),
            cab(
                "cabinet_full",
                "Full Height Filler",
                Some("\u{2303}\u{2325}\u{2318}2"),
                K::FullHeightFiller,
            ),
            cab(
                "countertop",
                "Custom Countertop",
                Some("\u{2303}\u{2325}\u{2318}3"),
                K::CustomCountertop,
            ),
            cab(
                "countertop",
                "Custom Backsplash",
                Some("\u{2303}\u{2325}\u{2318}4"),
                K::CustomBacksplash,
            ),
            cab(
                "countertop",
                "Custom Counter Hole",
                Some("\u{2303}\u{2325}\u{2318}5"),
                K::CounterHole,
            ),
            cab("soffit", "Soffit Polygon", None, K::SoffitPolygon),
            // Corner and blind cabinets have no Chief hotkey of their own.
            sep(cab(
                "cabinet_base",
                "Corner Base Cabinet",
                None,
                K::CornerBase,
            )),
            cab("cabinet_wall", "Corner Wall Cabinet", None, K::CornerWall),
            cab("cabinet_base", "Blind Base Cabinet", None, K::BlindBase),
            cab("cabinet_wall", "Blind Wall Cabinet", None, K::BlindWall),
            // Library types: Vanity, Pantry, Tall Oven and Refrigerator cabinets.
            sep(preset("cabinet_base", plan_cabinets::CabinetPreset::Vanity)),
            preset("cabinet_full", plan_cabinets::CabinetPreset::Pantry),
            preset("cabinet_full", plan_cabinets::CabinetPreset::TallOven),
            preset("cabinet_full", plan_cabinets::CabinetPreset::Refrigerator),
        ],
    )
}

/// A cabinet flyout entry that places a library type (Vanity, Pantry, ...).
fn preset(icon: &'static str, p: plan_cabinets::CabinetPreset) -> Item {
    item(
        icon,
        p.name(),
        Action::Custom(crate::tools::cabinet::preset_command(p)),
    )
}

/// An electrical flyout entry that starts the electrical tool in `v`.
fn elec(
    icon: &'static str,
    name: &'static str,
    key: &'static str,
    v: crate::tools::electrical::ElecVariant,
) -> Item {
    with_hotkey(
        item(icon, name, Action::SetTool(ToolId::ElectricalVariant(v))),
        key,
    )
}

pub fn electrical() -> Flyout {
    use crate::tools::electrical::ElecVariant as E;
    // Entries without a Chief hotkey.
    let plain = |icon: &'static str, v: E| {
        item(
            icon,
            v.name(),
            Action::SetTool(ToolId::ElectricalVariant(v)),
        )
    };
    fly(
        "Electrical",
        vec![
            elec("outlet_110", "110V Outlet", "E, O", E::Outlet110),
            plain("outlet_110", E::Outlet110Quad),
            elec(
                "outlet_220",
                "220V Outlet",
                "\u{2303}\u{2325}\u{2318}7",
                E::Outlet220,
            ),
            elec(
                "outlet_110",
                "GFCI Outlet",
                "\u{2303}\u{2325}\u{21E7}\u{2318}Y",
                E::Gfci,
            ),
            plain("outlet_110", E::OutletFloor),
            plain("outlet_110", E::OutletWp),
            plain("outlet_220", E::OutletDedicated),
            elec("switch", "Switch", "E, S", E::Switch),
            plain("switch", E::Switch3Way),
            plain("switch", E::Switch4Way),
            plain("switch", E::SwitchDimmer),
            elec("light", "Light", "E, L", E::Light),
            plain("light", E::RecessedLight),
            plain("light", E::PendantLight),
            plain("light", E::WallLight),
            elec(
                "light",
                "Rope Light",
                "\u{2303}\u{2325}\u{21E7}\u{2318}A",
                E::RopeLight,
            ),
            plain("light", E::CeilingFan),
            plain("light", E::SmokeDetector),
            plain("light", E::CoDetector),
            plain("switch", E::Thermostat),
            plain("switch", E::Doorbell),
            plain("outlet_110", E::DataJack),
            plain("outlet_110", E::PhoneJack),
            plain("outlet_110", E::TvJack),
            plain("outlet_220", E::Panel),
            elec(
                "connect_electrical",
                "Electrical Connection",
                "E, C",
                E::Connection,
            ),
            elec(
                "outlet_110",
                "Auto Place Outlets",
                "E, A, O",
                E::AutoOutlets,
            ),
            plain("switch", E::AutoSwitches),
        ],
    )
}

pub fn stairs() -> Flyout {
    use crate::editor::stairs_view::StairKind as K;
    let st = |icon, k: K, key| {
        with_hotkey(
            item(icon, k.name(), Action::SetTool(ToolId::StairsVariant(k))),
            key,
        )
    };
    fly(
        "Stairs",
        vec![
            st("stairs", K::Draw, "\u{21E7}Y"),
            item(
                "stairs",
                K::Click.name(),
                Action::SetTool(ToolId::StairsVariant(K::Click)),
            ),
            st("stairs", K::Straight, "\u{2303}\u{2325}\u{21E7}\u{2318}B"),
            st("stairs", K::LShaped, "\u{2303}\u{2325}\u{21E7}\u{2318}C"),
            st("stairs", K::UShaped, "\u{2303}\u{2325}\u{21E7}\u{2318}D"),
            st(
                "stairs_curved",
                K::CurveLeft,
                "\u{2303}\u{2325}\u{21E7}\u{2318}E",
            ),
            st(
                "stairs_curved",
                K::CurveRight,
                "\u{2303}\u{2325}\u{21E7}\u{2318}F",
            ),
            item(
                "stairs_curved",
                K::Curved.name(),
                Action::SetTool(ToolId::StairsVariant(K::Curved)),
            ),
            item(
                "stairs_curved",
                K::Spiral.name(),
                Action::SetTool(ToolId::StairsVariant(K::Spiral)),
            ),
            st("landing", K::Landing, "\u{2303}\u{2325}\u{21E7}\u{2318}G"),
            st("ramp", K::Ramp, "\u{2303}\u{2325}\u{21E7}\u{2318}H"),
            item(
                "ramp",
                K::CurvedRamp.name(),
                Action::SetTool(ToolId::StairsVariant(K::CurvedRamp)),
            ),
            item(
                "stairs",
                K::ToDeck.name(),
                Action::SetTool(ToolId::StairsVariant(K::ToDeck)),
            ),
        ],
    )
}

/// The Fireplace flyout: a fireplace beside a wall, built into a wall, a
/// prefab one and a chimney on its own (CB-87).
pub fn fireplace() -> Flyout {
    use crate::tools::fireplace::FireplaceMode as M;
    let fp = |icon, m: M| item(icon, m.name(), Action::SetTool(ToolId::FireplaceVariant(m)));
    fly(
        "Fireplace",
        vec![
            fp("foundation", M::Masonry),
            fp("wall_exterior", M::InWall),
            fp("box", M::Prefab),
            fp("post", M::Chimney),
        ],
    )
}

pub fn floor() -> Flyout {
    fly(
        "Floor",
        vec![
            with_hotkey(
                item("floor_new", "Build New Floor", Action::BuildNewFloor),
                "\u{21E7}X",
            ),
            with_hotkey(
                item("floor_insert", "Insert New Floor", Action::InsertFloor),
                "\u{2303}\u{2325}\u{21E7}\u{2318}I",
            ),
            item(
                "floor_insert",
                "Insert New Floor Below",
                Action::InsertFloorBelow,
            ),
            item("floor_defaults", "Floor Defaults", Action::FloorDefaults),
            // Split-level floors: stairs where rooms of one floor meet at
            // different heights (R-86).
            item(
                "stairs",
                "Add Steps at Level Changes",
                Action::Custom(crate::editor::fireplace_view::cmd::ADD_STEPS),
            ),
            with_hotkey(
                item("foundation", "Build Foundation", Action::BuildFoundation),
                "\u{2318}F",
            ),
            with_hotkey(
                item("floor_delete", "Delete Current Floor", Action::DeleteFloor),
                "\u{2303}\u{2325}\u{21E7}\u{2318}J",
            ),
            with_hotkey(
                item(
                    "floor_delete",
                    "Delete Foundation",
                    Action::DeleteFoundation,
                ),
                "\u{2303}\u{2325}\u{21E7}\u{2318}K",
            ),
            with_hotkey(
                item(
                    "floor_up",
                    "Exchange With Floor Above",
                    Action::ExchangeFloorAbove,
                ),
                "\u{2303}\u{2325}\u{21E7}\u{2318}L",
            ),
            with_hotkey(
                item(
                    "floor_down",
                    "Exchange With Floor Below",
                    Action::ExchangeFloorBelow,
                ),
                "\u{2303}\u{2325}\u{21E7}\u{2318}M",
            ),
            det("floor_new", DetailsVariant::FloorMaterialRegion),
            found("floor_new", FoundationVariant::FloorHole),
            found("floor_new", FoundationVariant::CeilingHole),
            with_hotkey(
                item(
                    "floor_defaults",
                    "Rebuild Walls/Floors/Ceilings",
                    Action::RebuildAll,
                ),
                "F12",
            ),
        ],
    )
}

/// A Roof flyout entry that starts the roof tool in `mode`.
fn roof_k(
    icon: &'static str,
    name: &'static str,
    key: &'static str,
    mode: crate::tools::roof::RoofMode,
) -> Item {
    with_hotkey(
        item(icon, name, Action::SetTool(ToolId::RoofVariant(mode))),
        key,
    )
}

pub fn roof() -> Flyout {
    use crate::tools::roof::RoofMode as M;
    fly(
        "Roof",
        vec![
            roof_k("roof_plane", "Roof Plane", "Q", M::Plane),
            roof_k(
                "roof_build",
                "Build Roof",
                "\u{2303}\u{2325}\u{21E7}\u{2318}N",
                M::Build,
            ),
            roof_k(
                "roof_plane",
                "Ceiling Plane",
                "\u{2303}\u{2325}\u{21E7}\u{2318}U",
                M::Ceiling,
            ),
            item(
                "roof_plane",
                "Tray Ceiling Polyline",
                Action::SetTool(ToolId::TrayCeiling),
            ),
            item(
                "roof_plane",
                "Roof Baseline Polyline",
                Action::SetTool(ToolId::RoofBaseline),
            ),
            roof_k(
                "gable_line",
                "Gable/Roof Line",
                "\u{2303}\u{2325}\u{21E7}\u{2318}O",
                M::GableLine,
            ),
            roof_k(
                "roof_plane",
                "Roof Hole",
                "\u{2303}\u{2325}\u{21E7}\u{2318}T",
                M::Hole,
            ),
            roof_k(
                "skylight",
                "Skylight",
                "\u{2303}\u{2325}\u{21E7}\u{2318}S",
                M::Skylight,
            ),
            roof_k(
                "dormer",
                "Auto Dormer",
                "\u{2303}\u{2325}\u{21E7}\u{2318}Z",
                M::Dormer,
            ),
            roof_k(
                "dormer",
                "Auto Floating Dormer",
                "\u{2303}\u{2325}\u{21E7}\u{2318}R",
                M::FloatingDormer,
            ),
            item(
                "dormer",
                "Explode Dormer",
                Action::SetTool(ToolId::RoofVariant(M::Explode)),
            ),
            item(
                "roof_plane",
                "Roof Return",
                Action::SetTool(ToolId::RoofVariant(M::Return)),
            ),
            roof_k(
                "roof_plane",
                "Edit All Roof Planes",
                "\u{2303}\u{2325}\u{21E7}\u{2318}P",
                M::EditAll,
            ),
            item(
                "roof_plane",
                "Edit Roof Planes",
                Action::SetTool(ToolId::RoofVariant(M::Edit)),
            ),
            with_hotkey(
                item(
                    "roof_plane",
                    "Delete Roof Planes",
                    Action::Custom(crate::editor::dispatch::cmd::ROOF_DELETE_ALL),
                ),
                "\u{2303}\u{2325}\u{21E7}\u{2318}W",
            ),
            with_hotkey(
                item(
                    "roof_plane",
                    "Delete Ceiling Planes",
                    Action::Custom(crate::editor::dispatch::cmd::ROOF_DELETE_CEILINGS),
                ),
                "\u{2303}\u{2325}\u{21E7}\u{2318}X",
            ),
        ],
    )
}

pub fn trim() -> Flyout {
    fly(
        "Trim",
        vec![
            det("corner_boards", DetailsVariant::CornerBoards),
            det("corner_boards", DetailsVariant::AutoCornerBoards),
            det("quoins", DetailsVariant::Quoins),
            det("quoins", DetailsVariant::AutoQuoins),
            det("corner_boards", DetailsVariant::MoldingLine),
            det("corner_boards", DetailsVariant::MoldingPolyline),
            det("corner_boards", DetailsVariant::ReplaceMoldings),
        ],
    )
}

/// An entry of the framing tool.
fn fram(icon: &'static str, v: FramingVariant) -> Item {
    item(icon, v.name(), Action::SetTool(ToolId::FramingVariant(v)))
}

pub fn general_framing() -> Flyout {
    use FramingVariant as V;
    fly(
        "General Framing",
        vec![
            fram("framing_general", V::General),
            fram("post", V::Post),
            fram("post", V::PostWithFooting),
            fram("framing_general", V::Blocking),
            with_hotkey(
                item(
                    "framing_general",
                    "Build Framing",
                    Action::Framing(FramingCommand::Build),
                ),
                "\u{21E7}\u{2318}S",
            ),
            item(
                "framing_general",
                "Build All Framing",
                Action::Framing(FramingCommand::BuildAll),
            ),
            item(
                "framing_general",
                "Delete Framing",
                Action::Framing(FramingCommand::Delete),
            ),
            fram("marker", V::ReferenceMarker),
        ],
    )
}

pub fn floor_ceiling_framing() -> Flyout {
    use FramingVariant as V;
    fly(
        "Floor/Ceiling Framing",
        vec![
            fram("joist", V::Joist),
            fram("joist", V::JoistBlocking),
            fram("joist", V::JoistDirection),
            fram("beam", V::FloorCeilingBeam),
            fram("truss", V::FloorCeilingTruss),
            fram("joist", V::BearingLine),
        ],
    )
}

pub fn roof_framing() -> Flyout {
    use FramingVariant as V;
    fly(
        "Roof Framing",
        vec![
            fram("rafter", V::Rafter),
            fram("beam", V::RoofBeam),
            fram("framing_general", V::RoofBlocking),
            fram("beam", V::RoofPurlin),
            fram("truss", V::RoofTruss),
            fram("truss", V::GirderTruss),
            fram("truss", V::RoofTrussDirection),
            fram("truss", V::TrussBase),
        ],
    )
}

/// An entry of the details tool (Trim, Material Region, Wall Hatching, Deck,
/// Slab Footing and 3D Solid flyouts).
fn det(icon: &'static str, v: DetailsVariant) -> Item {
    item(icon, v.name(), Action::SetTool(ToolId::DetailsVariant(v)))
}

/// A Slab-flyout or platform-hole entry of the foundation tool.
fn found(icon: &'static str, v: FoundationVariant) -> Item {
    item(
        icon,
        v.name(),
        Action::SetTool(ToolId::FoundationVariant(v)),
    )
}

/// An entry of the images tool (Image and Distributed Objects flyouts, and
/// 3D Solid Feature).
fn img(icon: &'static str, m: ImageMode) -> Item {
    item(icon, m.name(), Action::SetTool(ToolId::ImagesVariant(m)))
}

pub fn slab() -> Flyout {
    fly(
        "Slab",
        vec![
            found("slab", FoundationVariant::Slab),
            found("slab", FoundationVariant::SlabFooting),
            found("slab", FoundationVariant::SlabHole),
            found("slab", FoundationVariant::SlabHoleFooting),
            found("slab", FoundationVariant::SquarePad),
            found("post", FoundationVariant::RoundPier),
        ],
    )
}

pub fn solid_3d() -> Flyout {
    fly(
        "3D Solid",
        vec![
            det("solid_3d", DetailsVariant::Solid3d),
            det("solid_3d", DetailsVariant::Face),
            det("solid_3d", DetailsVariant::Cone),
            det("cylinder", DetailsVariant::Cylinder),
            det("solid_3d", DetailsVariant::Pyramid),
            det("solid_3d", DetailsVariant::Sphere),
            sep(img("solid_3d", ImageMode::SolidFeature)),
        ],
    )
}

pub fn image() -> Flyout {
    fly(
        "Image",
        vec![
            img("drawing_sheet", ImageMode::CreateImage),
            img("drawing_sheet", ImageMode::BillboardImage),
            img("library_browser", ImageMode::ImageLibrary),
            sep(img("dim_manual", ImageMode::PointToPointResize)),
            img("crosshairs", ImageMode::RotateToAlign),
        ],
    )
}

pub fn distributed_objects() -> Flyout {
    fly(
        "Distributed Objects",
        vec![
            img("polyline", ImageMode::PolylinePath),
            img("polygon", ImageMode::PolylineRegion),
            img("spline", ImageMode::SplinePath),
            img("spline", ImageMode::SplineRegion),
        ],
    )
}

pub fn dimensions() -> Flyout {
    use crate::tools::dimension::DimMode as D;
    let dim = |icon, m: D, key: Option<&'static str>| {
        let it = item(icon, m.name(), Action::SetTool(ToolId::DimensionVariant(m)));
        match key {
            Some(k) => with_hotkey(it, k),
            None => it,
        }
    };
    fly(
        "Dimensions",
        vec![
            dim("dim_manual", D::Manual, Some("\u{2303}\u{2325}\u{2318}A")),
            dim("dim_end_to_end", D::EndToEnd, Some("D, E")),
            dim("dim_interior", D::Interior, Some("D, I")),
            dim(
                "dim_manual",
                D::PointToPoint,
                Some("\u{2303}\u{2325}\u{2318}B"),
            ),
            dim("dim_manual", D::Running, Some("\u{2303}\u{2325}\u{2318}C")),
            dim("dim_manual", D::Baseline, Some("\u{2303}\u{2325}\u{2318}D")),
            dim("dim_angular", D::Angular, Some("\u{2303}\u{2325}\u{2318}F")),
            dim("dim_angular", D::Radius, None),
            dim("dim_angular", D::ArcLength, None),
            dim(
                "dim_manual",
                D::Centerline,
                Some("\u{2303}\u{2325}\u{2318}G"),
            ),
            dim("dim_manual", D::TapeMeasure, Some("D, T, M")),
            sep(dim("dim_manual", D::ExtensionAdd, None)),
            dim("dim_manual", D::ExtensionDelete, None),
        ],
    )
}

pub fn auto_dimensions() -> Flyout {
    use crate::tools::dimension::DimMode as D;
    fly(
        "Automatic Dimensions",
        vec![
            with_hotkey(
                item(
                    "dim_auto_exterior",
                    D::AutoExterior.name(),
                    Action::SetTool(ToolId::DimensionVariant(D::AutoExterior)),
                ),
                "\u{21E7}A",
            ),
            item(
                "dim_auto_interior",
                D::AutoInterior.name(),
                Action::SetTool(ToolId::DimensionVariant(D::AutoInterior)),
            ),
            item(
                "dim_auto_interior",
                D::AutoNkba.name(),
                Action::SetTool(ToolId::DimensionVariant(D::AutoNkba)),
            ),
            with_hotkey(
                item(
                    "dim_auto_exterior",
                    D::AutoElevation.name(),
                    Action::SetTool(ToolId::DimensionVariant(D::AutoElevation)),
                ),
                "\u{2303}\u{2325}\u{2318}H",
            ),
            with_hotkey(
                item(
                    "dim_auto_interior",
                    D::AutoStoryPole.name(),
                    Action::SetTool(ToolId::DimensionVariant(D::AutoStoryPole)),
                ),
                "\u{2303}\u{2325}\u{2318}I",
            ),
        ],
    )
}

pub fn text_tools() -> Flyout {
    use crate::tools::text::TextMode as T;
    let txt = |icon, m: T, key| {
        with_hotkey(
            item(icon, m.name(), Action::SetTool(ToolId::TextVariant(m))),
            key,
        )
    };
    let txt_plain = |icon, m: T| item(icon, m.name(), Action::SetTool(ToolId::TextVariant(m)));
    let mut f = fly(
        "Text",
        vec![
            txt("text", T::Text, "Y"),
            txt("rich_text", T::RichText, "\u{2303}\u{2325}\u{2318}J"),
            txt("leader_line", T::LeaderLine, "\u{2325}L"),
            txt("arrow_line", T::ArrowLine, "\u{2325}A"),
            txt("callout", T::Callout, "\u{2303}\u{2325}\u{2318}K"),
            txt("marker", T::Marker, "\u{2303}\u{2325}\u{2318}M"),
            txt("note", T::Note, "\u{2303}\u{2325}\u{2318}N"),
            sep(txt_plain("note", T::NoteTypes)),
            txt_plain("text", T::Macros),
            txt_plain("text", T::TextStyles),
        ],
    );
    f.current = 2;
    f
}

/// The Schedule flyout: each entry starts the Schedule tool for a kind, and
/// a click in the plan places that schedule as a table that stays up to date.
pub fn schedule() -> Flyout {
    use crate::tools::schedule::{entry_name, FLYOUT_KINDS};
    let entries = FLYOUT_KINDS
        .iter()
        .map(|k| {
            let it = item(
                "note",
                entry_name(*k),
                Action::SetTool(ToolId::ScheduleVariant(*k)),
            );
            if *k == plan_core::schedules::ScheduleKind::General {
                sep(it)
            } else {
                it
            }
        })
        .collect();
    fly("Schedule", entries)
}

/// A CAD flyout entry that starts the CAD tool in `m`.
fn cad_item(icon: &'static str, m: crate::tools::cad::CadMode) -> Item {
    item(icon, m.name(), Action::SetTool(ToolId::CadVariant(m)))
}

pub fn points() -> Flyout {
    use crate::tools::cad::CadMode as C;
    fly(
        "Points",
        vec![
            cad_item("point", C::PlacePoint),
            cad_item("point", C::InputPoint),
            cad_item("point", C::PointMarker),
            cad_item("point", C::DeleteTempPoints),
        ],
    )
}

pub fn lines() -> Flyout {
    use crate::tools::cad::CadMode as C;
    fly(
        "Lines",
        vec![
            cad_item("line", C::Line),
            cad_item("line", C::InputLine),
            cad_item("arrow_line", C::LineArrow),
            cad_item("polyline", C::Polyline),
            item(
                "line",
                "Construction Line",
                Action::SetTool(ToolId::ConstructionLine),
            ),
        ],
    )
}

pub fn arcs() -> Flyout {
    use crate::tools::cad::CadMode as C;
    fly(
        "Arcs",
        vec![
            cad_item("arc", C::Arc),
            cad_item("arc", C::InputArc),
            cad_item("arc", C::ArcArrow),
        ],
    )
}

pub fn circles() -> Flyout {
    use crate::tools::cad::CadMode as C;
    fly(
        "Circles",
        vec![
            with_hotkey(cad_item("circle", C::Circle), "K"),
            cad_item("circle", C::CircleAboutCenter),
            cad_item("ellipse", C::Ellipse),
            cad_item("ellipse", C::Oval),
        ],
    )
}

pub fn boxes() -> Flyout {
    use crate::tools::cad::CadMode as C;
    fly(
        "Boxes",
        vec![
            with_hotkey(cad_item("rect_polyline", C::RectPolyline), "\u{21E7}P"),
            cad_item("box", C::Box),
            cad_item("polygon", C::Polygon),
            cad_item("box", C::CrossBox),
            cad_item("box", C::BlockingBox),
            cad_item("box", C::Insulation),
        ],
    )
}

pub fn cad_blocks() -> Flyout {
    use crate::tools::cad::CadMode as C;
    fly(
        "CAD Blocks",
        vec![
            cad_item("point", C::AddInsertionPoint),
            cad_item("point", C::AddBackoffPoint),
            cad_item("box", C::MakeBlock),
            cad_item("box", C::EditBlock),
            cad_item("box", C::ExplodeBlock),
            with_hotkey(cad_item("box", C::BlockManagement), "V"),
        ],
    )
}

// Terrain menu groups. The doc lists Elevation, Modifier, Feature, Garden Bed,
// Grass Region, Driveway and Terrain Wall and Curb in full; Water Feature,
// Stepping Stone, Road, Sidewalk, Plant and Sprinkler "follow the same
// polyline/spline pattern", so they carry those two variants.

pub fn terrain_wall_curb() -> Flyout {
    fly(
        "Terrain Wall and Curb",
        vec![
            terr("wall_exterior", "Straight Terrain Wall", T::StraightWall),
            terr("wall_exterior", "Straight Terrain Curb", T::StraightCurb),
            terr("wall_curved", "Curved Terrain Wall", T::CurvedWall),
            terr("wall_curved", "Curved Terrain Curb", T::CurvedCurb),
            sep(terr(
                "wall_exterior",
                "Straight Retaining Wall",
                T::StraightRetainingWall,
            )),
            terr(
                "wall_curved",
                "Curved Retaining Wall",
                T::CurvedRetainingWall,
            ),
        ],
    )
}

use crate::tools::terrain::TerrainVariant as T;

/// A terrain entry that starts the terrain tool in `v`.
fn terr(icon: &'static str, name: &'static str, v: crate::tools::terrain::TerrainVariant) -> Item {
    item(icon, name, Action::SetTool(ToolId::TerrainVariant(v)))
}

/// North Pointer, Scale Bar and Building Pad: the site objects of the Terrain menu.
pub fn site_objects() -> Flyout {
    fly(
        "Site Objects",
        vec![
            terr("terrain", "North Pointer", T::NorthPointer),
            terr("terrain", "Scale Bar", T::ScaleBar),
            terr("terrain", "Building Pad", T::BuildingPad),
            sep(terr(
                "terrain",
                "Import Terrain Data\u{2026}",
                T::ImportData,
            )),
            terr("terrain", "Import GPS Data\u{2026}", T::ImportGps),
            terr(
                "terrain",
                "Terrain Cut and Fill Report\u{2026}",
                T::CutFillReport,
            ),
        ],
    )
}

pub fn elevation_data() -> Flyout {
    fly(
        "Elevation Data",
        vec![
            terr("terrain", "Terrain Perimeter", T::Perimeter),
            terr("elevation_line", "Elevation Line", T::ElevationLine),
            terr("elevation_line", "Elevation Point", T::ElevationPoint),
            terr("terrain", "Elevation Region", T::ElevationRegion),
            terr("spline", "Elevation Spline", T::ElevationSpline),
            terr("terrain", "Terrain Break", T::Break),
            terr("terrain", "Terrain Labels", T::TerrainLabels),
            sep(terr(
                "terrain",
                "Terrain Elevation Reference Point",
                T::ReferencePoint,
            )),
            terr(
                "terrain",
                "Remove Terrain Elevation Reference Point",
                T::RemoveReferencePoint,
            ),
            sep(terr("terrain", "Build Terrain", T::Build)),
        ],
    )
}

pub fn terrain_modifier() -> Flyout {
    fly(
        "Modifier",
        vec![
            terr(
                "terrain",
                "Hill",
                crate::tools::terrain::TerrainVariant::Hill,
            ),
            terr(
                "terrain",
                "Valley",
                crate::tools::terrain::TerrainVariant::Valley,
            ),
            terr(
                "terrain",
                "Raised Region",
                crate::tools::terrain::TerrainVariant::Raised,
            ),
            terr(
                "terrain",
                "Lowered Region",
                crate::tools::terrain::TerrainVariant::Lowered,
            ),
            terr(
                "terrain",
                "Flat Region (Cut/Fill)",
                crate::tools::terrain::TerrainVariant::Flat,
            ),
        ],
    )
}

pub fn terrain_feature() -> Flyout {
    fly(
        "Feature",
        vec![
            terr("terrain", "Rectangular Feature", T::RectFeature),
            terr("terrain", "Kidney Shaped Feature", T::KidneyFeature),
            terr("spline", "Spline Feature", T::SplineFeature),
            terr("polyline", "Polyline Feature", T::PolylineFeature),
            terr("terrain", "Round Feature", T::RoundFeature),
            terr(
                "terrain",
                "Terrain Hole",
                crate::tools::terrain::TerrainVariant::Hole,
            ),
        ],
    )
}

pub fn garden_bed() -> Flyout {
    fly(
        "Garden Bed",
        vec![
            terr("polyline", "Polyline Garden Bed", T::BedPolyline),
            terr("terrain", "Kidney Garden Bed", T::BedKidney),
            terr("spline", "Spline Garden Bed", T::BedSpline),
        ],
    )
}

pub fn grass_region() -> Flyout {
    fly(
        "Grass Region",
        vec![
            terr("polyline", "Polyline Grass Region", T::GrassPolyline),
            terr("terrain", "Kidney Grass Region", T::GrassKidney),
            terr("spline", "Spline Grass Region", T::GrassSpline),
        ],
    )
}

pub fn water_feature() -> Flyout {
    fly(
        "Water Feature",
        vec![
            terr("polyline", "Polyline Water Feature", T::WaterPolyline),
            terr("spline", "Spline Water Feature", T::WaterSpline),
            sep(terr("spline", "Stream", T::Stream)),
        ],
    )
}

pub fn stepping_stone() -> Flyout {
    fly(
        "Stepping Stone",
        vec![
            terr("polyline", "Polyline Stepping Stone", T::StonePolyline),
            terr("spline", "Spline Stepping Stone", T::StoneSpline),
        ],
    )
}

pub fn road() -> Flyout {
    fly(
        "Road",
        vec![
            terr(
                "road",
                "Straight Road",
                crate::tools::terrain::TerrainVariant::Road,
            ),
            terr("road", "Spline Road", T::SplineRoad),
            terr("road", "Polyline Road", T::PolylineRoad),
            terr("road", "Median", T::Median),
            terr("road", "Cul-de-sac", T::CulDeSac),
            sep(terr("road", "Auto Generate Sidewalk", T::AutoSidewalk)),
        ],
    )
}

pub fn driveway() -> Flyout {
    fly(
        "Driveway",
        vec![
            terr(
                "road",
                "Straight Driveway",
                crate::tools::terrain::TerrainVariant::Driveway,
            ),
            terr("road", "Spline Driveway", T::SplineDriveway),
            terr("road", "Polyline Driveway", T::PolylineDriveway),
        ],
    )
}

pub fn sidewalk() -> Flyout {
    fly(
        "Sidewalk",
        vec![
            terr(
                "road",
                "Straight Sidewalk",
                crate::tools::terrain::TerrainVariant::Sidewalk,
            ),
            terr("road", "Spline Sidewalk", T::SplineSidewalk),
            terr("road", "Polyline Sidewalk", T::PolylineSidewalk),
        ],
    )
}

/// Road Marking: a painted stripe laid on the ground or on a road.
pub fn road_marking() -> Flyout {
    fly(
        "Road Marking",
        vec![
            terr("road", "Polyline Road Marking", T::RoadMarking),
            terr("road", "Spline Road Marking", T::SplineRoadMarking),
        ],
    )
}

pub fn plant() -> Flyout {
    fly(
        "Plant",
        vec![
            terr("terrain", "Polyline Plant", T::PlantPolyline),
            terr("terrain", "Spline Plant", T::PlantSpline),
            sep(terr("terrain", "Grow All Plants\u{2026}", T::GrowPlants)),
        ],
    )
}

pub fn sprinkler() -> Flyout {
    fly(
        "Sprinkler",
        vec![
            terr("terrain", "Polyline Sprinkler", T::SprinklerPolyline),
            terr("terrain", "Spline Sprinkler", T::SprinklerSpline),
            sep(terr(
                "terrain",
                "Polyline Sprinkler Line",
                T::SprinklerLinePolyline,
            )),
            terr(
                "terrain",
                "Spline Sprinkler Line",
                T::SprinklerLineSpline,
            ),
        ],
    )
}

/// One top-level submenu: either a single flyout, or a named menu that nests
/// several flyouts.
pub struct MenuGroup {
    pub name: Option<&'static str>,
    pub flyouts: Vec<Flyout>,
}

fn single(f: Flyout) -> MenuGroup {
    MenuGroup {
        name: None,
        flyouts: vec![f],
    }
}

/// The seventeen Build submenus, in Chief's order.
pub fn build_menu() -> Vec<MenuGroup> {
    vec![
        MenuGroup {
            name: Some("Wall"),
            flyouts: vec![straight_wall(), curved_wall()],
        },
        single(railing_deck()),
        single(fencing()),
        single(door()),
        single(window()),
        single(floor()),
        single(roof()),
        single(slab()),
        MenuGroup {
            name: Some("Framing"),
            flyouts: vec![general_framing(), floor_ceiling_framing(), roof_framing()],
        },
        single(trim()),
        single(stairs()),
        single(fireplace()),
        single(cabinet()),
        single(electrical()),
        single(solid_3d()),
        single(image()),
        single(distributed_objects()),
    ]
}

/// The Terrain menu's submenus (after its leading commands).
pub fn terrain_menu() -> Vec<Flyout> {
    vec![
        elevation_data(),
        terrain_modifier(),
        terrain_feature(),
        garden_bed(),
        grass_region(),
        water_feature(),
        stepping_stone(),
        terrain_wall_curb(),
        road(),
        driveway(),
        sidewalk(),
        road_marking(),
        plant(),
        sprinkler(),
        site_objects(),
    ]
}

// ----- 3D views and cameras (docs/parity/3d-views-cameras.md, C-1) -----

fn view3d(icon: &'static str, name: &'static str, cmd: View3dCommand) -> Item {
    item(icon, name, Action::View3d(cmd))
}

fn camera_tool(
    icon: &'static str,
    name: &'static str,
    v: crate::tools::camera::CameraVariant,
) -> Item {
    view3d(icon, name, View3dCommand::Tool(v))
}

/// The 3D view flyout: overview cameras (C-10, C-11, C-13, C-15).
pub fn view_3d() -> Flyout {
    use crate::tools::camera::CameraVariant as V;
    use plan_view3d::CameraMode;
    fly(
        "3D View",
        vec![
            with_hotkey(
                camera_tool("view_3d", "Perspective Full Overview", V::FullOverview),
                "\u{21E7}K",
            ),
            camera_tool("view_3d", "Perspective Floor Overview", V::FloorOverview),
            camera_tool("view_3d", "Doll House View", V::DollHouse),
            camera_tool("view_3d", "Glass House View", V::GlassHouse),
            sep(view3d(
                "view_plan",
                "Orthographic Full Overview",
                View3dCommand::Mode(CameraMode::PlanOverhead),
            )),
        ],
    )
}

/// Full Camera and the section cameras (C-4, C-17, C-19).
pub fn full_camera() -> Flyout {
    use crate::tools::camera::CameraVariant as V;
    fly(
        "Full Camera",
        vec![
            with_hotkey(
                camera_tool("camera_full", "Full Camera", V::FullCamera),
                "\u{21E7}J",
            ),
            camera_tool("camera_full", "Floor Camera", V::FloorCamera),
            camera_tool(
                "cross_section",
                "Cross Section/Elevation Camera",
                V::CrossSection,
            ),
            camera_tool(
                "cross_section",
                "Back-Clipped Cross Section",
                V::BackClippedSection,
            ),
            sep(camera_tool(
                "cross_section",
                "Wall Elevation Camera",
                V::WallElevation,
            )),
            camera_tool("cross_section", "Auto Elevations", V::AutoElevation),
            camera_tool(
                "cross_section",
                "Auto Back-Clipped Elevations",
                V::AutoBackclipped,
            ),
            camera_tool("cross_section", "Auto Interior Elevations", V::AutoInterior),
        ],
    )
}

/// Create Walkthrough Path, with Play and Record (C-1).
pub fn walkthrough() -> Flyout {
    use crate::tools::camera::CameraVariant as V;
    fly(
        "Create Walkthrough Path",
        vec![
            camera_tool("walkthrough", "Create Walkthrough Path", V::Walkthrough),
            sep(view3d(
                "walkthrough",
                "Play Walkthrough",
                View3dCommand::PlayWalkthrough,
            )),
            view3d(
                "walkthrough",
                "Record Walkthrough",
                View3dCommand::RecordWalkthrough,
            ),
        ],
    )
}

/// Add Lights and Adjust Lights (C-64).
pub fn add_lights() -> Flyout {
    use crate::tools::camera::CameraVariant as V;
    fly(
        "Add Lights",
        vec![
            camera_tool("add_lights", "Add Lights", V::AddLights),
            with_hotkey(
                view3d("add_lights", "Adjust Lights", View3dCommand::AdjustLights),
                "\u{2303}\u{2325}\u{2318}L",
            ),
        ],
    )
}

pub fn mouse_orbit() -> Flyout {
    fly(
        "Mouse-Orbit Camera",
        vec![view3d(
            "camera_orbit",
            "Mouse-Orbit Camera",
            View3dCommand::MouseOrbit,
        )],
    )
}

pub fn cross_section_slider() -> Flyout {
    fly(
        "Cross Section Slider",
        vec![view3d(
            "cross_section",
            "Cross Section Slider",
            View3dCommand::CrossSectionSlider,
        )],
    )
}

/// Rendering Techniques (C-45); the face shows the first entry.
pub fn rendering_techniques() -> Flyout {
    fly(
        "Rendering Techniques",
        plan_materials::RenderingTechnique::ALL
            .iter()
            .map(|t| view3d("render_standard", t.label(), View3dCommand::Technique(*t)))
            .collect(),
    )
}

// ----- the three bars -----

fn row1_slots() -> Vec<Slot> {
    use Slot::Separator as Sep;
    vec![
        Slot::Button(item("file_new", "New Plan", Action::FileNew)),
        Slot::Button(item("file_open", "Open Plan", Action::FileOpen)),
        Slot::Button(item("file_save", "Save", Action::FileSave)),
        Sep,
        Slot::Button(item(
            "file_print",
            "Print",
            Action::Layout(crate::shell::layout_window::LayoutCommand::PrintDialog),
        )),
        Slot::Button(item(
            "send_to_layout",
            "Send to Layout",
            Action::Layout(crate::shell::layout_window::LayoutCommand::SendToLayout),
        )),
        Sep,
        Slot::Button(with_hotkey(item("undo", "Undo", Action::Undo), "\u{2318}Z")),
        Slot::Button(with_hotkey(item("redo", "Redo", Action::Redo), "\u{2318}Y")),
        Slot::Button(item(
            "note",
            "Check Spelling",
            Action::Custom(crate::dialogs::spell_check::OPEN),
        )),
        Sep,
        Slot::Button(item(
            "preferences",
            "Preferences",
            Action::Custom(crate::dialogs::preferences::OPEN),
        )),
        Slot::Button(item(
            "help",
            "Launch Help",
            Action::Custom(crate::dialogs::app_info::HELP),
        )),
        Sep,
        Slot::Button(item(
            "view_edit",
            "Edit Active View",
            Action::Custom(crate::dialogs::plan_views::OPEN),
        )),
        Slot::Button(item(
            "view_save",
            "Save Active View",
            Action::Custom(crate::dialogs::plan_views::SAVE),
        )),
        Slot::Button(item(
            "view_save_as",
            "Save Active View As",
            Action::Custom(crate::dialogs::app_info::NEW_PLAN_VIEW),
        )),
        Slot::ViewSelector,
        Sep,
        button("display_options", "Display Options"),
        Slot::Button(item(
            "default_settings",
            "Default Settings",
            Action::DefaultSettings,
        )),
        button("plan_database", "Plan Database"),
        Slot::Button(item(
            "floor_defaults",
            "Floor Defaults",
            Action::FloorDefaults,
        )),
        Sep,
        Slot::Button(item("floor_down", "Down One Floor", Action::FloorDown)),
        Slot::FloorLabel,
        Slot::Button(item("floor_up", "Up One Floor", Action::FloorUp)),
        Sep,
        flyout_slot(view_3d()),
        flyout_slot(full_camera()),
        flyout_slot(mouse_orbit()),
        flyout_slot(cross_section_slider()),
        flyout_slot(walkthrough()),
        flyout_slot(rendering_techniques()),
        flyout_slot(add_lights()),
        Sep,
        flag_toggle("sun_angle", "Sun Angle", ViewFlag::SunAngle),
        Sep,
        Slot::Toggle(item(
            "material_painter",
            "Material Painter",
            Action::Custom(crate::tools::materials::PAINTER),
        )),
        Slot::Toggle(item(
            "material_eyedropper",
            "Material Eyedropper",
            Action::Custom(crate::tools::materials::EYEDROPPER),
        )),
        Slot::Toggle(item(
            "object_eyedropper",
            "Object Eyedropper",
            Action::Custom(crate::tools::materials::OBJECT_EYEDROPPER),
        )),
        Slot::Toggle(item(
            "delete_surface",
            "Delete Surface",
            Action::Custom(crate::tools::materials::ERASE),
        )),
        Slot::Toggle(item(
            "adjust_material",
            "Adjust Material Definition",
            Action::Custom(crate::tools::materials::ADJUST_DEFINITION),
        )),
        // TODO parity: a dedicated icon (it borrows the Default Configuration glyph).
        Slot::Toggle(item(
            "config_default",
            "Use Default Material",
            Action::Custom(crate::tools::materials::USE_DEFAULT),
        )),
        Slot::Button(item(
            "material_editor",
            "Interactive Material Editor",
            Action::Custom(crate::tools::materials::LIST),
        )),
        Sep,
        toggle("config_default", "Default Configuration"),
        toggle("config_space_planning", "Space Planning Configuration"),
        toggle("config_extended", "Extended Tool Configuration"),
    ]
}

fn row2_slots() -> Vec<Slot> {
    use Slot::Separator as Sep;
    vec![
        Slot::Toggle(with_hotkey(
            item("select", "Select Objects", SELECT),
            "Space",
        )),
        Sep,
        flyout_slot(straight_wall()),
        flyout_slot(railing_deck()),
        flyout_slot(curved_wall()),
        Sep,
        flyout_slot(door()),
        flyout_slot(window()),
        Sep,
        flyout_slot(cabinet()),
        flyout_slot(electrical()),
        Sep,
        flyout_slot(stairs()),
        flyout_slot(floor()),
        Sep,
        flyout_slot(roof()),
        flyout_slot(trim()),
        flyout_slot(general_framing()),
        flyout_slot(floor_ceiling_framing()),
        flyout_slot(roof_framing()),
        flyout_slot(slab()),
        flyout_slot(solid_3d()),
        Sep,
        Slot::Button(item(
            "paste_hold",
            "Paste Hold Position",
            Action::Custom(crate::editor::edit_commands::ids::PASTE_HOLD),
        )),
        Sep,
        flyout_slot(dimensions()),
        flyout_slot(auto_dimensions()),
        Sep,
        flyout_slot(text_tools()),
        Slot::Toggle(item(
            "revision_cloud",
            "Revision Cloud",
            Action::SetTool(ToolId::CadVariant(
                crate::tools::cad::CadMode::RevisionCloud,
            )),
        )),
        flyout_slot(schedule()),
        Sep,
        flyout_slot(points()),
        flyout_slot(lines()),
        flyout_slot(arcs()),
        flyout_slot(circles()),
        flyout_slot(boxes()),
        Slot::Toggle(item(
            "spline",
            "Spline",
            Action::SetTool(ToolId::CadVariant(crate::tools::cad::CadMode::Spline)),
        )),
        Sep,
        Slot::Button(item(
            "auto_detail",
            "Auto Detail",
            Action::Custom(crate::tools::details::AUTO_DETAIL),
        )),
        Slot::Button(item(
            "cad_layer",
            "Current CAD Layer",
            Action::Custom(crate::dialogs::layer_sets::ACTIVE_LAYERS),
        )),
        Sep,
        painter_toggle("cad_layer", PainterMode::LayerPaint),
        painter_toggle("material_eyedropper", PainterMode::LayerEyedropper),
        painter_toggle("material_painter", PainterMode::ObjectPaint),
        painter_toggle("object_eyedropper", PainterMode::ObjectEyedropper),
    ]
}

/// A toggle of the Layer / Object Painter family (Tools menu).
fn painter_toggle(icon: &'static str, mode: PainterMode) -> Slot {
    Slot::Toggle(item(
        icon,
        mode.toolbar_name(),
        Action::SetTool(ToolId::PainterVariant(mode)),
    ))
}

fn view_slots() -> Vec<Slot> {
    use Slot::Separator as Sep;
    let dock = |icon, name, d| Slot::Toggle(item(icon, name, Action::ToggleDock(d)));
    vec![
        Slot::Toggle(with_hotkey(
            item(
                "library_browser",
                "Library Browser",
                Action::ToggleDock(Dock::Library),
            ),
            "\u{2318}L",
        )),
        dock("project_browser", "Project Browser", Dock::Project),
        dock(
            "layer_display",
            "Active Layer Display Options",
            Dock::LayerDisplay,
        ),
        Sep,
        Slot::Toggle(item(
            "zoom",
            "Zoom",
            Action::Custom(crate::shell::view_commands::ZOOM_WINDOW),
        )),
        Slot::Button(item("zoom_in", "Zoom In", Action::ZoomIn)),
        Slot::Button(item("zoom_out", "Zoom Out", Action::ZoomOut)),
        Slot::Button(item("zoom_undo", "Undo Zoom", Action::UndoZoom)),
        Slot::Button(item(
            "fill_selected",
            "Fill Window Selected Objects",
            Action::Custom(crate::dialogs::app_info::FILL_SELECTED),
        )),
        Slot::Button(item(
            "fill_building",
            "Fill Window Building Only",
            Action::Custom(crate::shell::view_commands::FILL_BUILDING),
        )),
        Slot::Button(with_hotkey(
            item("fill_window", "Fill Window", Action::FillWindow),
            "\u{2303}F",
        )),
        Sep,
        Slot::Toggle(with_hotkey(
            item("pan", "Pan Window", Action::TogglePan),
            "H",
        )),
        Sep,
        flag_toggle(
            "reference_display",
            "Reference Display",
            ViewFlag::ReferenceDisplay,
        ),
        Slot::Button(item(
            "reference_display",
            "Reference Display Options",
            Action::Custom(crate::dialogs::reference_display::CHANGE),
        )),
        Slot::Button(item(
            "reference_display",
            "Swap Floor/Reference",
            Action::Custom(crate::dialogs::reference_display::SWAP),
        )),
        flag_toggle("crosshairs", "Crosshairs", ViewFlag::Crosshairs),
        Slot::Toggle(with_hotkey(
            item("color", "Color", Action::ToggleFlag(ViewFlag::Color)),
            "F8",
        )),
        flag_toggle("line_weights", "Line Weights", ViewFlag::LineWeights),
        flag_toggle("drawing_sheet", "Drawing Sheet", ViewFlag::DrawingSheet),
        flag_toggle("print_preview", "Print Preview", ViewFlag::PrintPreview),
        flag_toggle(
            "temp_dimensions",
            "Temporary Dimensions",
            ViewFlag::TemporaryDimensions,
        ),
        flag_toggle("connect_cad", "Connect CAD Segments", ViewFlag::ConnectCad),
        flag_toggle("arc_centers", "Arc Centers and Ends", ViewFlag::ArcCenters),
    ]
}

// ----- rendering -----

/// Draws a horizontal bar and returns the actions triggered this frame.
pub fn row(ui: &mut egui::Ui, slots: &mut [Slot], state: &BarState) -> Vec<Action> {
    let mut out = Vec::new();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(GAP_PX, 0.0);
        // The configured set (config.rs) decides which buttons show.
        if !config::draw_bar(ui, slots, state, &mut out) {
            for (i, slot) in slots.iter_mut().enumerate() {
                show_slot(ui, i, slot, state, &mut out);
            }
        }
    });
    config::draw_custom_rows(ui, slots, state, &mut out);
    out
}

/// Draws a vertical bar and returns the actions triggered this frame.
pub fn column(ui: &mut egui::Ui, slots: &mut [Slot], state: &BarState) -> Vec<Action> {
    let mut out = Vec::new();
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, GAP_PX);
        if !config::draw_bar(ui, slots, state, &mut out) {
            for (i, slot) in slots.iter_mut().enumerate() {
                show_slot(ui, i, slot, state, &mut out);
            }
        }
    });
    out
}

/// A flyout turned into a submenu titled with its group name: icon, name and
/// hotkey per entry. Unimplemented entries are shown disabled.
pub fn flyout_menu(ui: &mut egui::Ui, f: &Flyout, state: &BarState, out: &mut Vec<Action>) {
    ui.menu_button(f.group, |ui| flyout_items(ui, f, state, out));
}

/// The entries of a flyout as menu items (no enclosing submenu).
pub fn flyout_items(ui: &mut egui::Ui, f: &Flyout, state: &BarState, out: &mut Vec<Action>) {
    for (i, e) in f.entries.iter().enumerate() {
        if e.sep_before && i > 0 {
            ui.separator();
        }
        let usable = !is_dimmed(e, is_enabled(e, state));
        let btn = entry_button(e, is_active(&e.action, state), state.brightness);
        if ui.add_enabled(usable, btn).clicked() {
            out.push(e.action);
            ui.close_menu();
        }
    }
}

fn show_slot(
    ui: &mut egui::Ui,
    idx: usize,
    slot: &mut Slot,
    state: &BarState,
    out: &mut Vec<Action>,
) {
    match slot {
        Slot::Separator => separator(ui),
        Slot::Button(it) => single_button(ui, it, false, state, out),
        Slot::Toggle(it) => {
            let active = is_active(&it.action, state);
            single_button(ui, it, active, state, out);
        }
        Slot::Flyout(f) => show_flyout(ui, idx, f, state, out),
        Slot::ViewSelector => {
            egui::ComboBox::from_id_salt("view_selector")
                .width(VIEW_SELECTOR_PX)
                .selected_text(state.view_name)
                .show_ui(ui, |ui| {
                    for (i, v) in state.views.iter().enumerate() {
                        let on = v.name == state.view_name;
                        if ui.selectable_label(on, &v.name).clicked() && !on {
                            out.push(Action::PlanView(i));
                        }
                    }
                    if state.views.is_empty() {
                        let _ = ui.selectable_label(true, state.view_name);
                    }
                });
        }
        Slot::FloorLabel => {
            ui.add_sized(
                [FLOOR_LABEL_PX, BUTTON_PX],
                egui::Label::new(egui::RichText::new((state.floor + 1).to_string()).strong()),
            );
        }
    }
}

fn separator(ui: &mut egui::Ui) {
    let horizontal = ui.layout().is_horizontal();
    let size = if horizontal {
        Vec2::new(1.0, BUTTON_PX)
    } else {
        Vec2::new(BUTTON_PX, 1.0)
    };
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let stroke = Stroke::new(1.0_f32, SEPARATOR_COLOR);
    if horizontal {
        let x = rect.center().x;
        ui.painter().line_segment(
            [
                egui::pos2(x, rect.top() + 2.0),
                egui::pos2(x, rect.bottom() - 2.0),
            ],
            stroke,
        );
    } else {
        let y = rect.center().y;
        ui.painter().line_segment(
            [
                egui::pos2(rect.left() + 2.0, y),
                egui::pos2(rect.right() - 2.0, y),
            ],
            stroke,
        );
    }
}

fn is_active(action: &Action, state: &BarState) -> bool {
    match action {
        Action::SetTool(t) => *t == state.tool,
        Action::TogglePan => state.tool == ToolId::Pan,
        Action::ToggleFlag(f) => state.flags.contains(f),
        Action::ToggleDock(d) => state.dock == Some(*d),
        // The Material Painter family are modes of the 3D view.
        Action::Custom(id) => crate::tools::materials::is_mode_active(id),
        _ => false,
    }
}

fn is_enabled(it: &Item, state: &BarState) -> bool {
    match it.action {
        Action::FloorDown => it.enabled && state.floor > 0,
        Action::FloorUp => it.enabled && state.floor + 1 < state.floor_count,
        Action::Undo => state.undo_label.is_some(),
        Action::Redo => state.redo_label.is_some(),
        _ => it.enabled,
    }
}

fn is_dimmed(it: &Item, enabled: bool) -> bool {
    !enabled || matches!(it.action, Action::NotImplemented(_))
}

/// Hotkey text as drawn in tooltips and menus. egui's bundled fonts lack the
/// macOS modifier glyphs (shift, control, option, delete, tab), so those are
/// spelled out; the tables keep Chief's symbols.
pub fn pretty_hotkey(k: &str) -> String {
    pretty_hotkey_for(k, cfg!(target_os = "macos"))
}

/// [`pretty_hotkey`] for either platform family. Without a Command key
/// (Windows, Linux) Chief's Command and Control both read `Ctrl+`.
pub fn pretty_hotkey_for(k: &str, mac: bool) -> String {
    let mut out = String::new();
    for c in k.chars() {
        match c {
            '\u{21E7}' => out.push_str("Shift+"),
            // Control and Command are one key off the Mac.
            '\u{2303}' if mac || !k.contains('\u{2318}') => out.push_str("Ctrl+"),
            '\u{2303}' => {}
            '\u{2325}' => out.push_str("Alt+"),
            '\u{2318}' if mac => out.push_str("Cmd+"),
            '\u{2318}' => out.push_str("Ctrl+"),
            '\u{2326}' => out.push_str("Del"),
            '\u{21E5}' => out.push_str("Tab"),
            c => out.push(c),
        }
    }
    out
}

/// The tooltip: the name and hotkey, then a one-line description from the
/// manual's command tables when it has one.
fn tooltip(it: &Item) -> String {
    let base = match it.hotkey {
        Some(k) => format!("{}  ({})", it.name, pretty_hotkey(k)),
        None => it.name.to_string(),
    };
    if matches!(it.action, Action::NotImplemented(_)) {
        format!("{base} \u{2014} not yet implemented")
    } else if let Some(d) = crate::shell::tooltips::describe(it.name) {
        format!("{base}\n{d}")
    } else {
        base
    }
}

/// Alpha of disabled icons (45 %).
const DIMMED_ALPHA: u8 = 115;

/// Image tint: full white scaled by the UI brightness, or white at 45 % alpha.
fn icon_tint(dimmed: bool, brightness: f32) -> Color32 {
    if dimmed {
        let g = (DIMMED_ALPHA as f32 * brightness).round() as u8;
        Color32::from_rgba_premultiplied(g, g, g, DIMMED_ALPHA)
    } else {
        scale(Color32::WHITE, brightness)
    }
}

/// A menu / flyout-list row: icon, name, hotkey on the right.
fn entry_button(e: &Item, selected: bool, brightness: f32) -> egui::Button<'static> {
    let enabled = e.enabled;
    let img = Image::new(icons::icon(e.icon))
        .fit_to_exact_size(Vec2::splat(ICON_PX))
        .tint(icon_tint(is_dimmed(e, enabled), brightness));
    let mut btn = egui::Button::image_and_text(img, e.name).selected(selected);
    if let Some(k) = e.hotkey {
        btn = btn.shortcut_text(pretty_hotkey(k));
    }
    btn
}

/// What a toolbar zone looks like right now.
#[derive(Clone, Copy, Default)]
struct ZoneState {
    active: bool,
    hovered: bool,
    dimmed: bool,
    /// Has keyboard focus (Tab).
    focused: bool,
}

/// Draws the rings of a zone: a light ring on hover, an accent ring on the
/// active tool and a thick white ring on keyboard focus. All of them clear
/// 3:1 against the panel (see `theme::chrome_colors`).
fn paint_rings(ui: &egui::Ui, zone: Rect, st: ZoneState, brightness: f32) {
    let chrome = crate::theme::current_chrome();
    let ring = |color: Color32, width: f32, inset: f32| {
        ui.painter().rect_stroke(
            zone.shrink(inset),
            3.0,
            Stroke::new(width, scale(color, brightness)),
            egui::StrokeKind::Inside,
        );
    };
    if st.active {
        ring(chrome.accent, 2.0, 0.5);
    } else if st.hovered && !st.dimmed {
        ring(chrome.outline, 1.5, 0.5);
    }
    if st.focused {
        ring(chrome.text, 2.0, 0.0);
    }
}

/// Paints the 20 px glyph centered in `zone`, with the highlight and rings
/// around it.
fn paint_glyph(ui: &egui::Ui, zone: Rect, id: &str, st: ZoneState, brightness: f32) {
    let chrome = crate::theme::current_chrome();
    let (active, hovered, dimmed) = (st.active, st.hovered, st.dimmed);
    if active {
        ui.painter()
            .rect_filled(zone, 3.0, scale(chrome.active, brightness));
    } else if hovered && !dimmed {
        ui.painter()
            .rect_filled(zone, 3.0, scale(chrome.hover, brightness));
    } else if crate::dialogs::preferences::icon_halo() && !dimmed {
        // Preferences > Appearance > Icon halo: a plate behind the icon.
        ui.painter()
            .rect_filled(zone.shrink(1.0), 4.0, scale(HALO_FILL, brightness));
    }
    paint_rings(ui, zone, st, brightness);
    let img = Image::new(icons::icon(id)).tint(icon_tint(dimmed, brightness));
    img.paint_at(
        ui,
        // Preferences > Appearance > Icon size.
        Rect::from_center_size(
            zone.center(),
            Vec2::splat(crate::dialogs::preferences::pages::icon_px()),
        ),
    );
}

fn single_button(
    ui: &mut egui::Ui,
    it: &Item,
    active: bool,
    state: &BarState,
    out: &mut Vec<Action>,
) {
    let enabled = is_enabled(it, state);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(BUTTON_PX), sense);
    paint_glyph(
        ui,
        rect,
        it.icon,
        ZoneState {
            active,
            hovered: resp.hovered(),
            dimmed: is_dimmed(it, enabled),
            focused: resp.has_focus(),
        },
        state.brightness,
    );
    if resp.clicked() {
        out.push(it.action);
    }
    if it.action == Action::PlanCheck {
        code_badge(ui, rect);
    }
    resp.on_hover_text(tooltip(it));
}

/// The live Plan Check count (errors and warnings) as a small red badge on
/// the corner of the Plan Check button; nothing while the count is zero.
fn code_badge(ui: &egui::Ui, rect: Rect) {
    let n = crate::editor::code::badge_count();
    if n == 0 {
        return;
    }
    let text = if n > 99 {
        "99+".to_string()
    } else {
        n.to_string()
    };
    let center = egui::pos2(rect.right() - 6.0, rect.top() + 6.0);
    let r = if text.len() > 1 { 8.0 } else { 6.5 };
    ui.painter()
        .circle_filled(center, r, Color32::from_rgb(0xC6, 0x28, 0x28));
    ui.painter().text(
        center,
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(9.0),
        Color32::WHITE,
    );
}

fn show_flyout(
    ui: &mut egui::Ui,
    idx: usize,
    fly: &mut Flyout,
    state: &BarState,
    out: &mut Vec<Action>,
) {
    let cur = fly.entries[fly.current];
    let enabled = is_enabled(&cur, state);
    let dimmed = is_dimmed(&cur, enabled);
    let active = is_active(&cur.action, state);
    let id = ui.id().with(("flyout", idx));
    let popup_id = id.with("popup");

    let (rect, whole) =
        ui.allocate_exact_size(Vec2::new(BUTTON_PX + ARROW_PX, BUTTON_PX), Sense::hover());
    let icon_zone = Rect::from_min_size(rect.min, Vec2::splat(BUTTON_PX));
    let arrow_zone = Rect::from_min_max(egui::pos2(rect.min.x + BUTTON_PX, rect.min.y), rect.max);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let icon_resp = ui.interact(icon_zone, id.with("icon"), sense);
    let arrow_resp = ui.interact(arrow_zone, id.with("arrow"), sense);

    paint_glyph(
        ui,
        icon_zone,
        cur.icon,
        ZoneState {
            active,
            hovered: icon_resp.hovered(),
            dimmed,
            focused: icon_resp.has_focus(),
        },
        state.brightness,
    );
    if arrow_resp.hovered() && enabled {
        ui.painter().rect_filled(
            arrow_zone,
            3.0,
            scale(crate::theme::current_chrome().hover, state.brightness),
        );
    }
    paint_rings(
        ui,
        arrow_zone,
        ZoneState {
            hovered: arrow_resp.hovered(),
            dimmed: !enabled,
            focused: arrow_resp.has_focus(),
            ..ZoneState::default()
        },
        state.brightness,
    );
    let c = arrow_zone.center();
    let tint = if dimmed {
        Color32::from_white_alpha(DIMMED_ALPHA)
    } else {
        scale(Color32::WHITE, state.brightness)
    };
    ui.painter().add(Shape::convex_polygon(
        vec![
            egui::pos2(c.x - 3.0, c.y - 1.5),
            egui::pos2(c.x + 3.0, c.y - 1.5),
            egui::pos2(c.x, c.y + 2.5),
        ],
        tint,
        Stroke::NONE,
    ));

    if icon_resp.clicked() {
        out.push(cur.action);
    }
    if arrow_resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    let tip = tooltip(&cur);
    icon_resp.on_hover_text(tip.clone());
    arrow_resp.on_hover_text(tip);

    let mut picked = None;
    egui::popup_below_widget(
        ui,
        popup_id,
        &whole,
        PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(FLYOUT_MIN_WIDTH);
            // Larger hit targets than the menu bar's rows.
            ui.spacing_mut().interact_size.y = FLYOUT_ROW_PX;
            ui.spacing_mut().button_padding = Vec2::new(8.0, 5.0);
            ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                for (i, e) in fly.entries.iter().enumerate() {
                    if e.sep_before && i > 0 {
                        ui.separator();
                    }
                    let en = is_enabled(e, state);
                    let btn = entry_button(e, i == fly.current, state.brightness);
                    if ui.add_enabled(en, btn).clicked() {
                        picked = Some(i);
                    }
                }
            });
        },
    );
    if let Some(i) = picked {
        fly.current = i;
        out.push(fly.entries[i].action);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_hotkeys_use_glyphs_the_bundled_font_has() {
        let fonts = egui::epaint::text::Fonts::new(1.0, 2048, egui::FontDefinitions::default());
        let font = egui::FontId::proportional(14.0);
        let mut all: Vec<&str> = Vec::new();
        for g in build_menu() {
            for f in g.flyouts {
                all.extend(f.entries.iter().filter_map(|e| e.hotkey));
            }
        }
        all.extend(["\u{2326}", "\u{21E7}Space", "\u{2303}\u{21E7}\u{21E5}"]);
        for k in all {
            let text = pretty_hotkey(k);
            assert!(fonts.has_glyphs(&font, &text), "missing glyph in {text:?}");
        }
        assert_eq!(
            pretty_hotkey_for("\u{2303}\u{2325}\u{2318}6", true),
            "Ctrl+Alt+Cmd+6"
        );
        assert_eq!(pretty_hotkey("D, H"), "D, H");
        // No Command key off the Mac: Command (and Control) read as Ctrl.
        assert_eq!(pretty_hotkey_for("\u{2318}S", false), "Ctrl+S");
        assert_eq!(
            pretty_hotkey_for("\u{2303}\u{2325}\u{2318}6", false),
            "Alt+Ctrl+6"
        );
        assert_eq!(pretty_hotkey_for("\u{2318}S", true), "Cmd+S");
    }

    fn press_seq(h: &mut Hotkeys, keys: &[Key], at: Instant) -> Vec<Action> {
        let mut out = Vec::new();
        for k in keys {
            h.press(*k, &Modifiers::NONE, at, &mut out);
        }
        out
    }

    #[test]
    fn d_then_h_places_hinged_door() {
        let mut h = Hotkeys::default();
        let t = Instant::now();
        assert!(press_seq(&mut h, &[Key::D], t).is_empty());
        assert_eq!(h.pending_label().as_deref(), Some("D, ..."));
        assert_eq!(press_seq(&mut h, &[Key::H], t), vec![DOOR]);
        assert!(h.pending_label().is_none());
    }

    #[test]
    fn sequence_times_out() {
        let mut h = Hotkeys::default();
        let t = Instant::now();
        press_seq(&mut h, &[Key::D], t);
        h.expire(t + SEQUENCE_TIMEOUT + Duration::from_millis(1));
        assert!(h.pending_label().is_none());
        // H after the timeout is the plain Pan hotkey, not the door.
        let later = t + SEQUENCE_TIMEOUT + Duration::from_millis(2);
        assert_eq!(press_seq(&mut h, &[Key::H], later), vec![Action::TogglePan]);
    }

    #[test]
    fn shift_keys_are_distinct_from_plain_keys() {
        let mut h = Hotkeys::default();
        let t = Instant::now();
        let mut out = Vec::new();
        let shift = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        h.press(Key::Q, &shift, t, &mut out);
        h.press(Key::Q, &Modifiers::NONE, t, &mut out);
        assert_eq!(out[0], EXTERIOR_WALL);
        assert_eq!(
            out[1],
            Action::SetTool(ToolId::RoofVariant(crate::tools::roof::RoofMode::Plane))
        );
    }

    #[test]
    fn every_flyout_entry_is_unique_per_group_and_dimmed_unless_built() {
        for g in build_menu() {
            for f in g.flyouts {
                let mut names: Vec<&str> = f.entries.iter().map(|e| e.name).collect();
                let n = names.len();
                names.sort_unstable();
                names.dedup();
                assert_eq!(names.len(), n, "duplicate entry in {}", f.group);
                for e in &f.entries {
                    if let Action::NotImplemented(name) = e.action {
                        assert_eq!(name, e.name);
                    }
                }
            }
        }
    }

    /// Every flyout of the Build, Terrain and CAD menus, with the group's
    /// name.
    fn all_flyouts() -> Vec<Flyout> {
        let mut v = Vec::new();
        for g in build_menu() {
            v.extend(g.flyouts);
        }
        v.extend(terrain_menu());
        v.extend([
            points(),
            lines(),
            arcs(),
            circles(),
            boxes(),
            cad_blocks(),
            dimensions(),
            auto_dimensions(),
            text_tools(),
            view_3d(),
            full_camera(),
        ]);
        v
    }

    /// The flyouts of tools that exist; every entry maps to a live action
    /// except the names in `ALLOWED_NOT_IMPLEMENTED`. The other flyouts
    /// (framing, slabs, curved walls, trim, terrain extras, ...) belong to
    /// tools that are not built yet and are only printed.
    const BUILT_GROUPS: &[&str] = &[
        "Straight Wall",
        "Curved Wall",
        "Railing and Deck",
        "Fencing",
        "Stairs",
        "Roof",
        "Cabinet",
        "Electrical",
        "Dimensions",
        "Automatic Dimensions",
        "Text",
        "Points",
        "Lines",
        "Arcs",
        "Circles",
        "Boxes",
        "CAD Blocks",
        "3D View",
        "Full Camera",
        "Elevation Data",
        "Modifier",
        "Terrain Wall and Curb",
        "Feature",
        "Garden Bed",
        "Grass Region",
        "Water Feature",
        "Stepping Stone",
        "Road",
        "Driveway",
        "Sidewalk",
        "Plant",
        "Sprinkler",
        "3D Solid",
        "Image",
        "Distributed Objects",
    ];

    /// Entries of the built groups that are still `NotImplemented`.
    const ALLOWED_NOT_IMPLEMENTED: &[&str] = &[];

    #[test]
    fn flyout_entries_are_live_except_the_allowlist() {
        let mut unexpected: Vec<(&str, &str)> = Vec::new();
        let mut still_stubs: Vec<&str> = Vec::new();
        for f in all_flyouts() {
            for e in &f.entries {
                if !matches!(e.action, Action::NotImplemented(_)) {
                    continue;
                }
                println!("NotImplemented: {} / {}", f.group, e.name);
                if !BUILT_GROUPS.contains(&f.group) {
                    continue;
                }
                if ALLOWED_NOT_IMPLEMENTED.contains(&e.name) {
                    still_stubs.push(e.name);
                } else {
                    unexpected.push((f.group, e.name));
                }
            }
        }
        assert!(unexpected.is_empty(), "{unexpected:?}");
        // The allowlist stays honest: an entry that went live comes off it.
        let now_live: Vec<&str> = ALLOWED_NOT_IMPLEMENTED
            .iter()
            .copied()
            .filter(|n| !still_stubs.contains(n))
            .collect();
        assert!(
            now_live.is_empty(),
            "live now; remove from ALLOWED_NOT_IMPLEMENTED: {now_live:?}"
        );
    }

    #[test]
    fn variant_entries_are_unique_and_named_after_their_tool() {
        for f in all_flyouts() {
            for e in &f.entries {
                let name = match e.action {
                    Action::SetTool(ToolId::DimensionVariant(m)) => m.name(),
                    Action::SetTool(ToolId::TextVariant(m)) => m.name(),
                    Action::SetTool(ToolId::CadVariant(m)) => m.name(),
                    Action::SetTool(ToolId::StairsVariant(m)) => m.name(),
                    _ => continue,
                };
                assert_eq!(name, e.name, "{}", f.group);
            }
        }
    }

    #[test]
    fn the_3d_and_camera_flyouts_have_no_unimplemented_entries() {
        let mut names = Vec::new();
        for f in [
            view_3d(),
            full_camera(),
            mouse_orbit(),
            cross_section_slider(),
            walkthrough(),
            add_lights(),
            rendering_techniques(),
        ] {
            for e in &f.entries {
                assert!(
                    !matches!(e.action, Action::NotImplemented(_)),
                    "{} / {} is still a stub",
                    f.group,
                    e.name
                );
                names.push(e.name);
            }
        }
        for want in [
            "Wall Elevation Camera",
            "Auto Elevations",
            "Auto Back-Clipped Elevations",
            "Create Walkthrough Path",
            "Play Walkthrough",
            "Record Walkthrough",
            "Add Lights",
            "Adjust Lights",
        ] {
            assert!(names.contains(&want), "missing {want}");
        }
        // Both flyouts sit on row 1 where the old placeholders were.
        let row = row1_slots();
        for group in ["Create Walkthrough Path", "Add Lights"] {
            let fly = row
                .iter()
                .find_map(|s| match s {
                    Slot::Flyout(f) if f.group == group => Some(f),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no {group} flyout on row 1"));
            assert!(!matches!(fly.entries[0].action, Action::NotImplemented(_)));
        }
        // The camera tools are tools, the lights dialog is a 3D command.
        let adjust = add_lights()
            .entries
            .into_iter()
            .find(|e| e.name == "Adjust Lights")
            .unwrap();
        assert_eq!(adjust.action, Action::View3d(View3dCommand::AdjustLights));
    }

    #[test]
    fn toolbar_buttons_are_28_to_32_px() {
        const {
            assert!(BUTTON_PX >= 28.0 && BUTTON_PX <= 32.0);
            assert!(ICON_PX < BUTTON_PX);
            // Flyout rows are bigger hit targets than the 24 pt menu rows.
            assert!(FLYOUT_ROW_PX >= 30.0 && FLYOUT_MIN_WIDTH >= 260.0);
        }
    }

    #[test]
    fn tooltips_carry_the_hotkey_and_a_manual_description() {
        let it = Item {
            icon: "wall_exterior",
            name: "Straight Exterior Wall",
            hotkey: Some("\u{21E7}Q"),
            action: EXTERIOR_WALL,
            enabled: true,
            sep_before: false,
        };
        let tip = tooltip(&it);
        let mut lines = tip.lines();
        assert_eq!(lines.next(), Some("Straight Exterior Wall  (Shift+Q)"));
        assert_eq!(
            lines.next(),
            Some("Uses the Default Settings exterior wall type and height.")
        );
        // Without a manual entry the tooltip is just the name.
        let plain = Item {
            name: "Some Unlisted Tool",
            hotkey: None,
            ..it
        };
        assert_eq!(tooltip(&plain), "Some Unlisted Tool");
    }

    fn frame_with(ctx: &egui::Context, events: Vec<egui::Event>, slots: &mut [Slot]) {
        let flags = HashSet::new();
        let state = BarState {
            tool: ToolId::Select,
            flags: &flags,
            dock: None,
            floor: 0,
            floor_count: 2,
            view_name: "Floor Plan",
            views: &[],
            brightness: 1.0,
            undo_label: None,
            redo_label: None,
            hotkeys: None,
        };
        let raw = egui::RawInput {
            events,
            screen_rect: Some(Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                Vec2::new(1600.0, 400.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = row(ui, slots, &state);
            });
        });
    }

    fn tab(shift: bool) -> egui::Event {
        egui::Event::Key {
            key: Key::Tab,
            physical_key: Some(Key::Tab),
            pressed: true,
            repeat: false,
            modifiers: if shift {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
        }
    }

    #[test]
    fn tab_walks_the_toolbar_buttons_and_draws_a_focus_ring() {
        let ctx = egui::Context::default();
        crate::icons::install(&ctx);
        crate::theme::apply_settings(&ctx, &crate::theme::AppSettings::default());
        let mut slots = row1_slots();
        frame_with(&ctx, vec![], &mut slots);
        assert_eq!(ctx.memory(|m| m.focused()), None);
        let mut seen = Vec::new();
        for _ in 0..4 {
            frame_with(&ctx, vec![tab(false)], &mut slots);
            frame_with(&ctx, vec![], &mut slots);
            seen.push(ctx.memory(|m| m.focused()).expect("Tab focuses a button"));
        }
        let mut unique = seen.clone();
        unique.sort_by_key(|i| format!("{i:?}"));
        unique.dedup();
        assert_eq!(
            unique.len(),
            4,
            "Tab visits a new widget each time: {seen:?}"
        );
        // Shift+Tab goes back.
        frame_with(&ctx, vec![tab(true)], &mut slots);
        frame_with(&ctx, vec![], &mut slots);
        assert_eq!(ctx.memory(|m| m.focused()), Some(seen[2]));
    }
}
