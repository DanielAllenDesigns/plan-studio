//! Chief Architect-style toolbars: data tables plus a small renderer.
//!
//! The three bars mirror `docs/chief-x18-toolbars.md` (row 1, row 2 and the
//! right-edge view bar). Every flyout's entries come from
//! `docs/chief-x18-subtools.md` and are built by one function per group
//! (e.g. [`straight_wall`]); the toolbar, the Build/Terrain/CAD menus and the
//! hotkey tooltips all read those same tables. Rendering returns [`Action`]s;
//! the app applies them.

use crate::icons;
use crate::theme::{scale, CanvasTheme};
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
const ACTIVE_FILL: Color32 = Color32::from_rgb(0x5A, 0x5A, 0x5A);
const HOVER_FILL: Color32 = Color32::from_rgb(0x4A, 0x4A, 0x4A);
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
}

impl Dock {
    pub fn title(self) -> &'static str {
        match self {
            Dock::Library => "Library Browser",
            Dock::Project => "Project Browser",
            Dock::LayerDisplay => "Active Layer Display Options",
        }
    }
}

/// Everything the UI can ask the app to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    SetTool(ToolId),
    /// The wall flyout's currently selected variant (hotkey alias `2`).
    CurrentWall,
    FileNew,
    FileOpen,
    FileSave,
    FileSaveAs,
    /// File > Templates > Save Current Defaults as My Template...
    SaveTemplate,
    /// File > Templates > Reset to Chief X18 Template
    ResetTemplate,
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
    NotImplemented(&'static str),
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
    /// Global UI brightness (0.6..=1.0), applied to icon tints and fills.
    pub brightness: f32,
    /// Label of the step Undo would revert ("Move Wall"); `None` when there is none.
    pub undo_label: Option<&'a str>,
    pub redo_label: Option<&'a str>,
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
    // Chief hotkeys for tools that are not built yet.
    bind(
        "\u{21E7}Y",
        &[Key::Y],
        SHIFT,
        Action::NotImplemented("Draw Stairs"),
    ),
    bind(
        "\u{21E7}T",
        &[Key::T],
        SHIFT,
        Action::NotImplemented("Base Cabinet"),
    ),
    bind(
        "\u{21E7}A",
        &[Key::A],
        SHIFT,
        Action::NotImplemented("Auto Exterior Dimensions"),
    ),
    bind("Q", &[Key::Q], NONE, Action::NotImplemented("Roof Plane")),
    bind("Y", &[Key::Y], NONE, Action::NotImplemented("Text")),
    bind("K", &[Key::K], NONE, Action::NotImplemented("Circle")),
    bind(
        "\u{21E7}P",
        &[Key::P],
        SHIFT,
        Action::NotImplemented("Rectangular Polyline"),
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
#[derive(Default)]
pub struct Hotkeys {
    pending: Vec<Key>,
    since: Option<Instant>,
}

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

/// An unimplemented entry that has a Chief hotkey.
fn todo_k(icon: &'static str, name: &'static str, key: &'static str) -> Item {
    with_hotkey(todo(icon, name), key)
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
            todo("wall_foundation", "Straight Foundation Wall"),
            todo("wall_pony", "Straight Pony Wall"),
            todo("wall_exterior", "Straight Glass Wall"),
            todo("wall_pony", "Straight Glass Pony Wall"),
            todo("wall_half", "Straight Half-Wall"),
            todo("wall_room_divider", "Room Divider"),
            todo("slab", "Slab Footing"),
            todo("wall_hatch", "Wall Hatching"),
            todo("wall_hatch", "Wall Material Region"),
        ],
    )
}

pub fn curved_wall() -> Flyout {
    fly(
        "Curved Wall",
        vec![
            todo("wall_curved", "Curved Exterior Wall"),
            todo("wall_curved_interior", "Curved Interior Wall"),
            todo("wall_curved", "Curved Foundation Wall"),
            todo("wall_curved", "Curved Pony Wall"),
            todo("wall_curved", "Curved Half-Wall"),
        ],
    )
}

pub fn railing_deck() -> Flyout {
    fly(
        "Railing and Deck",
        vec![
            todo_k("railing", "Straight Railing", "\u{2318}Q"),
            todo("railing_curved", "Curved Railing"),
            sep(todo("deck_railing", "Straight Deck Railing")),
            todo("deck_railing", "Curved Deck Railing"),
            sep(todo("deck_edge", "Straight Deck Edge")),
            todo("deck_edge", "Curved Deck Edge"),
            sep(todo("deck_edge", "Polygon Shaped Deck")),
        ],
    )
}

pub fn fencing() -> Flyout {
    fly(
        "Fencing",
        vec![
            todo("railing", "Straight Fencing"),
            todo("railing_curved", "Curved Fencing"),
        ],
    )
}

pub fn door() -> Flyout {
    fly(
        "Door",
        vec![
            with_hotkey(item("door_hinged", "Hinged Door", DOOR), "D, H"),
            todo_k("doorway", "Doorway", "D, W"),
            todo_k("door_sliding", "Sliding Door", "S, D"),
            todo_k("door_pocket", "Pocket Door", "D, P"),
            todo_k("door_bifold", "Bifold Door", "\u{2303}\u{2325}\u{2318}O"),
            todo_k("door_barn", "Barn Door", "\u{2303}\u{2325}\u{2318}P"),
            todo_k("door_hinged", "Fixed Door", "\u{2303}\u{2325}\u{2318}R"),
            todo_k("door_garage", "Garage Door", "G, D"),
            todo_k("door_hinged", "Shower Door", "\u{2303}\u{2325}\u{2318}Q"),
        ],
    )
}

pub fn window() -> Flyout {
    fly(
        "Window",
        vec![
            with_hotkey(item("window", "Window", WINDOW), "\u{21E7}W"),
            todo_k("window_bay", "Bay Window", "\u{2303}\u{2325}\u{2318}S"),
            todo_k("window_bow", "Bow Window", "\u{2303}\u{2325}\u{2318}T"),
            todo_k("window_box", "Box Window", "\u{2303}\u{2325}\u{2318}U"),
            todo_k("pass_through", "Pass-Through", "\u{2303}\u{2325}\u{2318}V"),
            todo_k("pass_through", "Wall Niche", "\u{2303}\u{2325}\u{2318}W"),
        ],
    )
}

pub fn cabinet() -> Flyout {
    fly(
        "Cabinet",
        vec![
            todo_k("cabinet_base", "Base Cabinet", "\u{21E7}T"),
            todo_k("cabinet_wall", "Wall Cabinet", "\u{2318}T"),
            todo_k("cabinet_full", "Full Height", "\u{2303}\u{2325}\u{2318}X"),
            todo_k("soffit", "Soffit", "T"),
            todo_k("shelf", "Shelf", "\u{2303}\u{2325}\u{2318}Y"),
            todo_k("partition", "Partition", "\u{2303}\u{2325}\u{2318}Z"),
            todo_k("cabinet_base", "Base Filler", "\u{2303}\u{2325}\u{2318}0"),
            todo_k("cabinet_wall", "Wall Filler", "\u{2303}\u{2325}\u{2318}1"),
            todo_k(
                "cabinet_full",
                "Full Height Filler",
                "\u{2303}\u{2325}\u{2318}2",
            ),
            todo_k(
                "countertop",
                "Custom Countertop",
                "\u{2303}\u{2325}\u{2318}3",
            ),
            todo_k(
                "countertop",
                "Custom Backsplash",
                "\u{2303}\u{2325}\u{2318}4",
            ),
            todo_k(
                "countertop",
                "Custom Counter Hole",
                "\u{2303}\u{2325}\u{2318}5",
            ),
        ],
    )
}

pub fn electrical() -> Flyout {
    fly(
        "Electrical",
        vec![
            todo_k("outlet_110", "110V Outlet", "E, O"),
            todo_k("outlet_220", "220V Outlet", "\u{2303}\u{2325}\u{2318}7"),
            todo_k(
                "outlet_110",
                "GFCI Outlet",
                "\u{2303}\u{2325}\u{21E7}\u{2318}Y",
            ),
            todo_k("light", "Light", "E, L"),
            todo_k("light", "Rope Light", "\u{2303}\u{2325}\u{21E7}\u{2318}A"),
            todo_k("switch", "Switch", "E, S"),
            todo_k("connect_electrical", "Electrical Connection", "E, C"),
            todo_k("outlet_110", "Auto Place Outlets", "E, A, O"),
        ],
    )
}

pub fn stairs() -> Flyout {
    fly(
        "Stairs",
        vec![
            todo_k("stairs", "Draw Stairs", "\u{21E7}Y"),
            todo_k(
                "stairs",
                "Straight Stairs",
                "\u{2303}\u{2325}\u{21E7}\u{2318}B",
            ),
            todo_k(
                "stairs",
                "L-Shaped Stair",
                "\u{2303}\u{2325}\u{21E7}\u{2318}C",
            ),
            todo_k(
                "stairs",
                "U-Shaped Stair",
                "\u{2303}\u{2325}\u{21E7}\u{2318}D",
            ),
            todo_k(
                "stairs_curved",
                "Curve to Left",
                "\u{2303}\u{2325}\u{21E7}\u{2318}E",
            ),
            todo_k(
                "stairs_curved",
                "Curve to Right",
                "\u{2303}\u{2325}\u{21E7}\u{2318}F",
            ),
            todo_k("landing", "Landing", "\u{2303}\u{2325}\u{21E7}\u{2318}G"),
            todo_k("ramp", "Draw Ramp", "\u{2303}\u{2325}\u{21E7}\u{2318}H"),
        ],
    )
}

pub fn floor() -> Flyout {
    fly(
        "Floor",
        vec![
            todo_k("floor_new", "Build New Floor", "\u{21E7}X"),
            todo_k(
                "floor_insert",
                "Insert New Floor",
                "\u{2303}\u{2325}\u{21E7}\u{2318}I",
            ),
            todo_k("foundation", "Build Foundation", "\u{2318}F"),
            todo_k(
                "floor_delete",
                "Delete Current Floor",
                "\u{2303}\u{2325}\u{21E7}\u{2318}J",
            ),
            todo_k(
                "floor_delete",
                "Delete Foundation",
                "\u{2303}\u{2325}\u{21E7}\u{2318}K",
            ),
            todo_k(
                "floor_up",
                "Exchange With Floor Above",
                "\u{2303}\u{2325}\u{21E7}\u{2318}L",
            ),
            todo_k(
                "floor_down",
                "Exchange With Floor Below",
                "\u{2303}\u{2325}\u{21E7}\u{2318}M",
            ),
            todo("floor_new", "Floor Material Region"),
            todo("floor_new", "Hole in Floor Platform"),
            todo("floor_new", "Hole in Ceiling Platform"),
            todo_k("floor_defaults", "Rebuild Walls/Floors/Ceilings", "F12"),
        ],
    )
}

pub fn roof() -> Flyout {
    fly(
        "Roof",
        vec![
            todo_k("roof_plane", "Roof Plane", "Q"),
            todo_k(
                "roof_build",
                "Build Roof",
                "\u{2303}\u{2325}\u{21E7}\u{2318}N",
            ),
            todo_k(
                "roof_plane",
                "Ceiling Plane",
                "\u{2303}\u{2325}\u{21E7}\u{2318}U",
            ),
            todo_k(
                "gable_line",
                "Gable/Roof Line",
                "\u{2303}\u{2325}\u{21E7}\u{2318}O",
            ),
            todo_k(
                "roof_plane",
                "Roof Hole",
                "\u{2303}\u{2325}\u{21E7}\u{2318}T",
            ),
            todo_k("skylight", "Skylight", "\u{2303}\u{2325}\u{21E7}\u{2318}S"),
            todo_k("dormer", "Auto Dormer", "\u{2303}\u{2325}\u{21E7}\u{2318}Z"),
            todo_k(
                "dormer",
                "Auto Floating Dormer",
                "\u{2303}\u{2325}\u{21E7}\u{2318}R",
            ),
            todo_k(
                "roof_plane",
                "Edit All Roof Planes",
                "\u{2303}\u{2325}\u{21E7}\u{2318}P",
            ),
            todo_k(
                "roof_plane",
                "Delete Roof Planes",
                "\u{2303}\u{2325}\u{21E7}\u{2318}W",
            ),
            todo_k(
                "roof_plane",
                "Delete Ceiling Planes",
                "\u{2303}\u{2325}\u{21E7}\u{2318}X",
            ),
        ],
    )
}

pub fn trim() -> Flyout {
    fly(
        "Trim",
        vec![
            todo("corner_boards", "Corner Boards"),
            todo("corner_boards", "Auto Place Corner Boards"),
            todo("quoins", "Quoins"),
            todo("quoins", "Auto Place Quoins"),
            todo("corner_boards", "Molding Line"),
            todo("corner_boards", "Molding Polyline"),
        ],
    )
}

pub fn general_framing() -> Flyout {
    fly(
        "General Framing",
        vec![
            todo("framing_general", "General Framing"),
            todo("post", "Post"),
            todo("post", "Post with Footing"),
            todo("framing_general", "Blocking"),
            todo_k("framing_general", "Build Framing", "\u{21E7}\u{2318}S"),
            todo("framing_general", "Build All Framing"),
            todo("marker", "Framing Reference Marker"),
        ],
    )
}

pub fn floor_ceiling_framing() -> Flyout {
    fly(
        "Floor/Ceiling Framing",
        vec![
            todo("joist", "Joist"),
            todo("joist", "Joist Blocking"),
            todo("joist", "Joist Direction"),
            todo("beam", "Floor/Ceiling Beam"),
            todo("truss", "Floor/Ceiling Truss"),
            todo("joist", "Bearing Line"),
        ],
    )
}

pub fn roof_framing() -> Flyout {
    fly(
        "Roof Framing",
        vec![
            todo("rafter", "Rafter"),
            todo("beam", "Roof Beam"),
            todo("framing_general", "Roof Blocking"),
            todo("beam", "Roof Purlin"),
            todo("truss", "Roof Truss"),
            todo("truss", "Girder Truss"),
            todo("truss", "Roof Truss Direction"),
            todo("truss", "Truss Base"),
        ],
    )
}

pub fn slab() -> Flyout {
    fly(
        "Slab",
        vec![
            todo("slab", "Slab"),
            todo("slab", "Slab with Footing"),
            todo("slab", "Slab Hole"),
            todo("slab", "Slab Hole with Footing"),
            todo("slab", "Square Pad"),
            todo("post", "Round Pier"),
        ],
    )
}

pub fn solid_3d() -> Flyout {
    fly(
        "3D Solid",
        vec![
            todo("solid_3d", "3D Solid"),
            todo("solid_3d", "Face"),
            todo("solid_3d", "Cone"),
            todo("cylinder", "Cylinder"),
            todo("solid_3d", "Pyramid"),
            todo("solid_3d", "Sphere"),
            sep(todo("solid_3d", "3D Solid Feature")),
        ],
    )
}

pub fn image() -> Flyout {
    fly(
        "Image",
        vec![
            todo("drawing_sheet", "Create Image"),
            todo("drawing_sheet", "Create Billboard Image"),
            todo("library_browser", "Create Image Library"),
        ],
    )
}

pub fn distributed_objects() -> Flyout {
    fly(
        "Distributed Objects",
        vec![
            todo("polyline", "Polyline Distribution Path"),
            todo("polygon", "Polyline Distribution Region"),
            todo("spline", "Spline Distribution Path"),
            todo("spline", "Spline Distribution Region"),
        ],
    )
}

pub fn dimensions() -> Flyout {
    fly(
        "Dimensions",
        vec![
            todo_k(
                "dim_manual",
                "Manual Dimension",
                "\u{2303}\u{2325}\u{2318}A",
            ),
            todo_k("dim_end_to_end", "End to End Dimension", "D, E"),
            todo_k("dim_interior", "Interior Dimension", "D, I"),
            todo_k(
                "dim_manual",
                "Point to Point Dimension",
                "\u{2303}\u{2325}\u{2318}B",
            ),
            todo_k(
                "dim_manual",
                "Running Dimension",
                "\u{2303}\u{2325}\u{2318}C",
            ),
            todo_k(
                "dim_manual",
                "Baseline Dimension",
                "\u{2303}\u{2325}\u{2318}D",
            ),
            todo_k(
                "dim_angular",
                "Angular Dimension",
                "\u{2303}\u{2325}\u{2318}F",
            ),
            todo_k(
                "dim_manual",
                "Centerline Dimension",
                "\u{2303}\u{2325}\u{2318}G",
            ),
            todo_k("dim_manual", "Tape Measure", "D, T, M"),
        ],
    )
}

pub fn auto_dimensions() -> Flyout {
    fly(
        "Automatic Dimensions",
        vec![
            todo_k("dim_auto_exterior", "Auto Exterior Dimensions", "\u{21E7}A"),
            todo_k(
                "dim_auto_exterior",
                "Auto Elevation Dimensions",
                "\u{2303}\u{2325}\u{2318}H",
            ),
            todo_k(
                "dim_auto_interior",
                "Auto Story Pole Dimensions",
                "\u{2303}\u{2325}\u{2318}I",
            ),
        ],
    )
}

/// Text Tools; the toolbar face starts on Leader Line.
pub fn text_tools() -> Flyout {
    let mut f = fly(
        "Text",
        vec![
            todo_k("text", "Text", "Y"),
            todo_k("rich_text", "Rich Text", "\u{2303}\u{2325}\u{2318}J"),
            todo_k("leader_line", "Leader Line", "\u{2325}L"),
            todo_k("arrow_line", "Text Line with Arrow", "\u{2325}A"),
            todo_k("callout", "Callout", "\u{2303}\u{2325}\u{2318}K"),
            todo_k("marker", "Marker", "\u{2303}\u{2325}\u{2318}M"),
            todo_k("note", "Note", "\u{2303}\u{2325}\u{2318}N"),
            sep(todo("note", "Note Type Management")),
            todo("text", "Text Macro Management"),
        ],
    );
    f.current = 2;
    f
}

pub fn points() -> Flyout {
    fly(
        "Points",
        vec![
            todo("point", "Place Point"),
            todo("point", "Input Point"),
            todo("point", "Point Marker"),
            todo("point", "Delete Temporary Points"),
        ],
    )
}

pub fn lines() -> Flyout {
    fly(
        "Lines",
        vec![
            todo("line", "Draw Line"),
            todo("line", "Input Line"),
            todo("arrow_line", "Line With Arrow"),
        ],
    )
}

pub fn arcs() -> Flyout {
    fly(
        "Arcs",
        vec![
            todo("arc", "Draw Arc"),
            todo("arc", "Input Arc"),
            todo("arc", "Arc With Arrow"),
        ],
    )
}

pub fn circles() -> Flyout {
    fly(
        "Circles",
        vec![
            todo_k("circle", "Circle", "K"),
            todo("circle", "Circle About Center"),
            todo("ellipse", "Ellipse"),
            todo("ellipse", "Oval"),
        ],
    )
}

pub fn boxes() -> Flyout {
    fly(
        "Boxes",
        vec![
            todo_k("rect_polyline", "Rectangular Polyline", "\u{21E7}P"),
            todo("box", "Box"),
            todo("polygon", "Regular Polygon"),
            todo("box", "Cross Box"),
            todo("box", "Blocking Box"),
            todo("box", "Insulation"),
        ],
    )
}

pub fn cad_blocks() -> Flyout {
    fly(
        "CAD Blocks",
        vec![
            todo("point", "Add Insertion Point"),
            todo("point", "Add Arrow Backoff Point"),
            todo("box", "Make CAD Block"),
            todo("box", "Edit CAD Block"),
            todo("box", "Explode CAD Block"),
            todo_k("box", "CAD Block Management", "V"),
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
            todo("wall_exterior", "Straight Terrain Wall"),
            todo("wall_exterior", "Straight Terrain Curb"),
            todo("wall_curved", "Curved Terrain Wall"),
            todo("wall_curved", "Curved Terrain Curb"),
        ],
    )
}

pub fn elevation_data() -> Flyout {
    fly(
        "Elevation Data",
        vec![
            todo("elevation_line", "Elevation Line"),
            todo("elevation_line", "Elevation Point"),
            todo("terrain", "Elevation Region"),
            todo("spline", "Elevation Spline"),
            todo("terrain", "Terrain Break"),
        ],
    )
}

pub fn terrain_modifier() -> Flyout {
    fly(
        "Modifier",
        vec![
            todo("terrain", "Hill"),
            todo("terrain", "Valley"),
            todo("terrain", "Raised Region"),
            todo("terrain", "Lowered Region"),
            todo("terrain", "Flat Region (Cut/Fill)"),
        ],
    )
}

pub fn terrain_feature() -> Flyout {
    fly(
        "Feature",
        vec![
            todo("terrain", "Rectangular Feature"),
            todo("terrain", "Kidney Shaped Feature"),
            todo("spline", "Spline Feature"),
            todo("terrain", "Terrain Hole"),
        ],
    )
}

pub fn garden_bed() -> Flyout {
    fly(
        "Garden Bed",
        vec![
            todo("polyline", "Polyline Garden Bed"),
            todo("terrain", "Kidney Garden Bed"),
            todo("spline", "Spline Garden Bed"),
        ],
    )
}

pub fn grass_region() -> Flyout {
    fly(
        "Grass Region",
        vec![
            todo("polyline", "Polyline Grass Region"),
            todo("terrain", "Kidney Grass Region"),
            todo("spline", "Spline Grass Region"),
        ],
    )
}

pub fn water_feature() -> Flyout {
    fly(
        "Water Feature",
        vec![
            todo("polyline", "Polyline Water Feature"),
            todo("spline", "Spline Water Feature"),
        ],
    )
}

pub fn stepping_stone() -> Flyout {
    fly(
        "Stepping Stone",
        vec![
            todo("polyline", "Polyline Stepping Stone"),
            todo("spline", "Spline Stepping Stone"),
        ],
    )
}

pub fn road() -> Flyout {
    fly(
        "Road",
        vec![todo("road", "Polyline Road"), todo("road", "Spline Road")],
    )
}

pub fn driveway() -> Flyout {
    fly(
        "Driveway",
        vec![
            todo("road", "Polyline Driveway"),
            todo("road", "Spline Driveway"),
        ],
    )
}

pub fn sidewalk() -> Flyout {
    fly(
        "Sidewalk",
        vec![
            todo("road", "Polyline Sidewalk"),
            todo("road", "Spline Sidewalk"),
        ],
    )
}

pub fn plant() -> Flyout {
    fly(
        "Plant",
        vec![
            todo("terrain", "Polyline Plant"),
            todo("terrain", "Spline Plant"),
        ],
    )
}

pub fn sprinkler() -> Flyout {
    fly(
        "Sprinkler",
        vec![
            todo("terrain", "Polyline Sprinkler"),
            todo("terrain", "Spline Sprinkler"),
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

/// The sixteen Build submenus, in Chief's order.
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
        plant(),
        sprinkler(),
    ]
}

// ----- the three bars -----

fn row1_slots() -> Vec<Slot> {
    use Slot::Separator as Sep;
    let one = |icon, name| flyout_slot(fly(name, vec![todo(icon, name)]));
    vec![
        Slot::Button(item("file_new", "New Plan", Action::FileNew)),
        Slot::Button(item("file_open", "Open Plan", Action::FileOpen)),
        Slot::Button(item("file_save", "Save", Action::FileSave)),
        Sep,
        button("file_print", "Print"),
        button("send_to_layout", "Send to Layout"),
        Sep,
        Slot::Button(with_hotkey(item("undo", "Undo", Action::Undo), "\u{2318}Z")),
        Slot::Button(with_hotkey(item("redo", "Redo", Action::Redo), "\u{2318}Y")),
        Sep,
        button("preferences", "Preferences"),
        button("help", "Launch Help"),
        Sep,
        button("view_edit", "Edit Active View"),
        button("view_save", "Save Active View"),
        button("view_save_as", "Save Active View As"),
        Slot::ViewSelector,
        Sep,
        button("display_options", "Display Options"),
        Slot::Button(item(
            "default_settings",
            "Default Settings",
            Action::DefaultSettings,
        )),
        button("plan_database", "Plan Database"),
        button("floor_defaults", "Floor Defaults"),
        Sep,
        Slot::Button(item("floor_down", "Down One Floor", Action::FloorDown)),
        Slot::FloorLabel,
        Slot::Button(item("floor_up", "Up One Floor", Action::FloorUp)),
        Sep,
        one("view_3d", "3D View"),
        one("camera_full", "Full Camera"),
        one("camera_orbit", "Mouse-Orbit Camera"),
        one("cross_section", "Cross Section Slider"),
        one("walkthrough", "Create Walkthrough Path"),
        one("render_standard", "Standard"),
        one("add_lights", "Add Lights"),
        Sep,
        flag_toggle("sun_angle", "Sun Angle", ViewFlag::SunAngle),
        Sep,
        button("material_painter", "Material Painter"),
        toggle("material_eyedropper", "Material Eyedropper"),
        toggle("object_eyedropper", "Object Eyedropper"),
        toggle("delete_surface", "Delete Surface"),
        toggle("adjust_material", "Adjust Material Definition"),
        toggle("material_editor", "Interactive Material Editor"),
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
        button("paste_hold", "Paste Hold Position"),
        Sep,
        flyout_slot(dimensions()),
        flyout_slot(auto_dimensions()),
        Sep,
        flyout_slot(text_tools()),
        toggle("revision_cloud", "Revision Cloud"),
        Sep,
        flyout_slot(points()),
        flyout_slot(lines()),
        flyout_slot(arcs()),
        flyout_slot(circles()),
        flyout_slot(boxes()),
        toggle("spline", "Spline"),
        Sep,
        button("auto_detail", "Auto Detail"),
        button("cad_layer", "Current CAD Layer"),
    ]
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
        toggle("zoom", "Zoom"),
        Slot::Button(item("zoom_in", "Zoom In", Action::ZoomIn)),
        Slot::Button(item("zoom_out", "Zoom Out", Action::ZoomOut)),
        Slot::Button(item("zoom_undo", "Undo Zoom", Action::UndoZoom)),
        button("fill_selected", "Fill Window Selected Objects"),
        button("fill_building", "Fill Window Building Only"),
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
        for (i, slot) in slots.iter_mut().enumerate() {
            show_slot(ui, i, slot, state, &mut out);
        }
    });
    out
}

/// Draws a vertical bar and returns the actions triggered this frame.
pub fn column(ui: &mut egui::Ui, slots: &mut [Slot], state: &BarState) -> Vec<Action> {
    let mut out = Vec::new();
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, GAP_PX);
        for (i, slot) in slots.iter_mut().enumerate() {
            show_slot(ui, i, slot, state, &mut out);
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
                    let _ = ui.selectable_label(true, state.view_name);
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
    let mut out = String::new();
    for c in k.chars() {
        match c {
            '\u{21E7}' => out.push_str("Shift+"),
            '\u{2303}' => out.push_str("Ctrl+"),
            '\u{2325}' => out.push_str("Alt+"),
            '\u{2318}' => out.push_str("Cmd+"),
            '\u{2326}' => out.push_str("Del"),
            '\u{21E5}' => out.push_str("Tab"),
            c => out.push(c),
        }
    }
    out
}

fn tooltip(it: &Item) -> String {
    let base = match it.hotkey {
        Some(k) => format!("{}  ({})", it.name, pretty_hotkey(k)),
        None => it.name.to_string(),
    };
    if matches!(it.action, Action::NotImplemented(_)) {
        format!("{base} \u{2014} not yet implemented")
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

/// Paints the 20 px glyph centered in `zone`, with the highlight behind it.
fn paint_glyph(
    ui: &egui::Ui,
    zone: Rect,
    id: &str,
    (active, hovered, dimmed): (bool, bool, bool),
    brightness: f32,
) {
    if active {
        ui.painter()
            .rect_filled(zone, 3.0, scale(ACTIVE_FILL, brightness));
    } else if hovered && !dimmed {
        ui.painter()
            .rect_filled(zone, 3.0, scale(HOVER_FILL, brightness));
    }
    let img = Image::new(icons::icon(id)).tint(icon_tint(dimmed, brightness));
    img.paint_at(
        ui,
        Rect::from_center_size(zone.center(), Vec2::splat(ICON_PX)),
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
        (active, resp.hovered(), is_dimmed(it, enabled)),
        state.brightness,
    );
    if resp.clicked() {
        out.push(it.action);
    }
    resp.on_hover_text(tooltip(it));
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
        (active, icon_resp.hovered(), dimmed),
        state.brightness,
    );
    if arrow_resp.hovered() && enabled {
        ui.painter()
            .rect_filled(arrow_zone, 3.0, scale(HOVER_FILL, state.brightness));
    }
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
            ui.set_min_width(260.0);
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
        assert_eq!(pretty_hotkey("\u{2303}\u{2325}\u{2318}6"), "Ctrl+Alt+Cmd+6");
        assert_eq!(pretty_hotkey("D, H"), "D, H");
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
        assert_eq!(out[1], Action::NotImplemented("Roof Plane"));
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
}
