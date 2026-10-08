//! Chief-style pull-down menu bar: File, Edit, Build, Terrain, Library, 3D,
//! CAD, Tools, View, Window, Help (docs/chief-x18-menus.md).
//!
//! Build, Terrain and CAD submenus are generated from the same flyout tables
//! as the toolbar ([`toolbar::flyout_menu`]). Items that are not built yet are
//! shown disabled so the target shape of the product is visible.

use crate::theme::{CanvasTheme, BRIGHTNESS_MAX, BRIGHTNESS_MIN};
use crate::toolbar::{self, Action, BarState, Dock, Tool, ViewFlag};
use eframe::egui;

/// Draws the menu bar contents; actions are appended to `out`.
pub fn bar(
    ui: &mut egui::Ui,
    state: &BarState,
    theme: CanvasTheme,
    brightness: &mut f32,
    out: &mut Vec<Action>,
) {
    ui.menu_button("File", |ui| file_menu(ui, out));
    ui.menu_button("Edit", |ui| edit_menu(ui, state, out));
    ui.menu_button("Build", |ui| build_menu(ui, state, out));
    ui.menu_button("Terrain", |ui| terrain_menu(ui, state, out));
    ui.menu_button("Library", |ui| {
        inert(
            ui,
            &[
                "Import Library (.calib, .calibz)\u{2026}",
                "-",
                "Get Additional Content\u{2026}",
                "Install Core Content",
                "Update Library Catalogs",
            ],
        )
    });
    ui.menu_button("3D", three_d_menu);
    ui.menu_button("CAD", |ui| cad_menu(ui, state, out));
    ui.menu_button("Tools", tools_menu);
    ui.menu_button("View", |ui| view_menu(ui, state, theme, brightness, out));
    ui.menu_button("Window", |ui| window_menu(ui, state, out));
    ui.menu_button("Help", |ui| help_menu(ui, out));
}

/// Disabled placeholder rows. `"-"` is a separator, a leading `+` shows the
/// row as checked, `"name\tshortcut"` adds a shortcut and a trailing `>` marks
/// a submenu.
fn inert(ui: &mut egui::Ui, items: &[&str]) {
    for raw in items {
        if *raw == "-" {
            ui.separator();
            continue;
        }
        let (checked, raw) = match raw.strip_prefix('+') {
            Some(rest) => (true, rest),
            None => (false, *raw),
        };
        let (name, shortcut) = raw.split_once('\t').unwrap_or((raw, ""));
        let (name, submenu) = match name.strip_suffix('>') {
            Some(n) => (n, true),
            None => (name, false),
        };
        let mut btn = egui::Button::new(name).selected(checked);
        if !shortcut.is_empty() {
            btn = btn.shortcut_text(toolbar::pretty_hotkey(shortcut));
        }
        if submenu {
            btn = btn.shortcut_text("\u{23F5}");
        }
        ui.add_enabled(false, btn);
    }
}

/// An implemented menu row.
fn live(
    ui: &mut egui::Ui,
    label: &str,
    shortcut: &str,
    checked: bool,
    action: Action,
    out: &mut Vec<Action>,
) {
    let mut btn = egui::Button::new(label).selected(checked);
    if !shortcut.is_empty() {
        btn = btn.shortcut_text(toolbar::pretty_hotkey(shortcut));
    }
    if ui.add(btn).clicked() {
        out.push(action);
        ui.close_menu();
    }
}

fn file_menu(ui: &mut egui::Ui, out: &mut Vec<Action>) {
    live(ui, "New Plan", "\u{2318}N", false, Action::FileNew, out);
    inert(ui, &["New Layout", "Templates>", "-"]);
    live(
        ui,
        "Open Plan\u{2026}",
        "\u{2318}O",
        false,
        Action::FileOpen,
        out,
    );
    inert(
        ui,
        &[
            "Open Layout\u{2026}",
            "Open Recent Documents>",
            "-",
            "Dashboard\u{2026}",
            "Download Sample Plans\u{2026}",
            "-",
            "Close View\t\u{2318}W",
            "Close All 3D Views",
            "Close All Views",
            "-",
        ],
    );
    live(ui, "Save", "\u{2318}S", false, Action::FileSave, out);
    live(ui, "Save As\u{2026}", "", false, Action::FileSaveAs, out);
    inert(
        ui,
        &[
            "Save As Template\u{2026}",
            "Save Thumbnail Image",
            "Show in Project Browser",
            "View File Information\u{2026}",
            "Manage Auto Archives\u{2026}",
            "-",
            "Export>",
            "Import>",
            "Print>",
            "-",
            "Send to Layout\u{2026}\tS, L",
            "-",
        ],
    );
    live(ui, "Quit", "", false, Action::Quit, out);
}

fn edit_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    inert(
        ui,
        &[
            "Undo\t\u{2318}Z",
            "Redo\t\u{2318}Y",
            "-",
            "Cut\t\u{2318}X",
            "Copy\t\u{2318}C",
            "Copy and Paste in Place\tC, P, P",
            "Paste>",
            "-",
            "Delete\t\u{2326}",
            "Delete Objects\u{2026}\t\u{21E7}Space",
            "-",
        ],
    );
    live(
        ui,
        "Select Objects",
        "Space",
        state.tool == Tool::Select,
        Action::SetTool(Tool::Select),
        out,
    );
    inert(
        ui,
        &[
            "Select All\t\u{2318}A",
            "-",
            "Snap Settings>",
            "Edit Behaviors>",
            "Arc Creation Modes>",
            "-",
            "Edit Area>",
            "Stretch CAD",
            "-",
            "Find/Replace Text\u{2026}",
            "Replace Fonts\u{2026}",
            "-",
            "Default Settings\u{2026}",
            "Reset to Defaults\u{2026}",
            "-",
            "AutoFill>",
            "Start Dictation",
            "Emoji & Symbols",
        ],
    );
}

fn build_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    for group in toolbar::build_menu() {
        match group.name {
            Some(name) => {
                ui.menu_button(name, |ui| {
                    for f in &group.flyouts {
                        toolbar::flyout_menu(ui, f, state, out);
                    }
                });
            }
            None => {
                for f in &group.flyouts {
                    toolbar::flyout_menu(ui, f, state, out);
                }
            }
        }
    }
}

fn terrain_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    inert(
        ui,
        &[
            "Create Terrain Perimeter",
            "-",
            "Terrain Specification\u{2026}",
            "Build Terrain",
            "Clear Terrain",
            "-",
        ],
    );
    for f in &toolbar::terrain_menu() {
        toolbar::flyout_menu(ui, f, state, out);
    }
}

fn three_d_menu(ui: &mut egui::Ui) {
    inert(
        ui,
        &[
            "Create Orthographic View>",
            "Create Perspective View>",
            "Create Auto Elevations>",
            "-",
            "Move Camera with Mouse>",
            "Move Camera with Keyboard>",
            "Move Camera>",
            "Orbit Camera>",
            "Tilt Camera>",
            "View Direction>",
            "Isometric Views>",
            "-",
            "Walkthroughs>",
            "-",
            "Materials>",
            "Material Painter>",
            "Adjust Materials>",
            "Adjust 3D Cladding>",
            "Material Builder\u{2026}",
            "-",
            "Lighting>",
            "Camera View Options>",
            "Rendering Techniques>",
            "Toggle Patterns",
            "Delete Surface",
            "Rebuild 3D",
            "-",
            "3D View Defaults\u{2026}\t\u{2318}1",
        ],
    );
}

fn cad_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    inert(ui, &["Current CAD Layer\u{2026}", "-"]);
    for f in [
        toolbar::points(),
        toolbar::lines(),
        toolbar::arcs(),
        toolbar::circles(),
        toolbar::boxes(),
    ] {
        toolbar::flyout_menu(ui, &f, state, out);
    }
    inert(ui, &["Revision Cloud", "Spline", "-"]);
    toolbar::flyout_menu(ui, &toolbar::dimensions(), state, out);
    toolbar::flyout_menu(ui, &toolbar::auto_dimensions(), state, out);
    ui.separator();
    toolbar::flyout_menu(ui, &toolbar::text_tools(), state, out);
    inert(ui, &["Patterns>"]);
    toolbar::flyout_menu(ui, &toolbar::cad_blocks(), state, out);
    inert(
        ui,
        &[
            "-",
            "Sun Angle",
            "North Pointer",
            "-",
            "Plan Footprint",
            "Auto Detail",
            "-",
            "CAD Block Management\u{2026}",
            "CAD Detail Management\u{2026}",
            "CAD Detail From View",
            "-",
            "CAD to Walls\u{2026}",
        ],
    );
}

fn tools_menu(ui: &mut egui::Ui) {
    inert(
        ui,
        &[
            "Layer Settings>",
            "Floor/Reference Display>",
            "Active View>",
            "Active Defaults\u{2026}",
            "-",
            "Checks>",
            "Toolbars and Hotkeys>",
            "Symbol>",
            "Space Planning>",
            "Plan Database>",
            "Time Tracker>",
            "Schedules>",
            "Materials List>",
            "Object Painter>",
            "Fill Style Painter>",
            "-",
            "Project Information\u{2026}",
            "Loan Calculator\u{2026}",
            "Ruby Console\u{2026}",
            "-",
            "Screen Capture>",
            "Color Chooser\u{2026}",
            "-",
            "New Plan View",
            "Rotate Plan View\u{2026}",
            "Reverse Plan",
        ],
    );
}

fn view_menu(
    ui: &mut egui::Ui,
    state: &BarState,
    theme: CanvasTheme,
    brightness: &mut f32,
    out: &mut Vec<Action>,
) {
    let flag = |f: ViewFlag| state.flags.contains(&f);
    let dock = |d: Dock| state.dock == Some(d);
    inert(ui, &["Refresh Display\tF5", "-"]);
    live(
        ui,
        "Library Browser",
        "\u{2318}L",
        dock(Dock::Library),
        Action::ToggleDock(Dock::Library),
        out,
    );
    live(
        ui,
        "Project Browser",
        "",
        dock(Dock::Project),
        Action::ToggleDock(Dock::Project),
        out,
    );
    inert(ui, &["Tool Palette"]);
    live(
        ui,
        "Active Layer Display Options",
        "",
        dock(Dock::LayerDisplay),
        Action::ToggleDock(Dock::LayerDisplay),
        out,
    );
    inert(
        ui,
        &[
            "Walkthrough Preview",
            "Action History",
            "-",
            "+Status Bar",
            "+Scrollbars",
            "+Toolbars",
            "-",
        ],
    );
    live(
        ui,
        "Color",
        "F8",
        flag(ViewFlag::Color),
        Action::ToggleFlag(ViewFlag::Color),
        out,
    );
    live(
        ui,
        "Crosshairs",
        "",
        flag(ViewFlag::Crosshairs),
        Action::ToggleFlag(ViewFlag::Crosshairs),
        out,
    );
    inert(
        ui,
        &[
            "Coordinate System Indicator \u{2013} Floating",
            "Coordinate System Indicator \u{2013} Fixed",
            "Coordinate System Indicator \u{2013} Origin",
            "-",
        ],
    );
    live(
        ui,
        "Reference Grid",
        "\u{21E7}F9",
        flag(ViewFlag::ReferenceGrid),
        Action::ToggleFlag(ViewFlag::ReferenceGrid),
        out,
    );
    inert(ui, &["Angle Snap Grid"]);
    live(
        ui,
        "Temporary Dimensions",
        "",
        flag(ViewFlag::TemporaryDimensions),
        Action::ToggleFlag(ViewFlag::TemporaryDimensions),
        out,
    );
    live(
        ui,
        "Arc Centers and Ends",
        "",
        flag(ViewFlag::ArcCenters),
        Action::ToggleFlag(ViewFlag::ArcCenters),
        out,
    );
    ui.separator();
    live(
        ui,
        "Line Weights",
        "",
        flag(ViewFlag::LineWeights),
        Action::ToggleFlag(ViewFlag::LineWeights),
        out,
    );
    live(
        ui,
        "Drawing Sheet",
        "",
        flag(ViewFlag::DrawingSheet),
        Action::ToggleFlag(ViewFlag::DrawingSheet),
        out,
    );
    inert(ui, &["Watermark", "-", "Enter Full Screen", "-"]);

    // Plan Studio additions: low-glare canvas themes and a global dimmer.
    ui.menu_button("Canvas Theme", |ui| {
        for t in CanvasTheme::ALL {
            if ui.radio(theme == t, t.label()).clicked() {
                out.push(Action::SetTheme(t));
                ui.close_menu();
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("UI Brightness");
        ui.add(egui::Slider::new(brightness, BRIGHTNESS_MIN..=BRIGHTNESS_MAX).fixed_decimals(2));
    });
}

fn window_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    inert(ui, &["Zoom\t\u{21E7}Z"]);
    live(ui, "Zoom Out", "-", false, Action::ZoomOut, out);
    live(ui, "Zoom In", "+", false, Action::ZoomIn, out);
    live(ui, "Undo Zoom", "", false, Action::UndoZoom, out);
    inert(
        ui,
        &["Fill Window Building Only", "Fill Window Selected Objects"],
    );
    live(
        ui,
        "Fill Window",
        "\u{2303}F",
        false,
        Action::FillWindow,
        out,
    );
    live(
        ui,
        "Pan Window",
        "H",
        state.tool == Tool::Pan,
        Action::TogglePan,
        out,
    );
    inert(
        ui,
        &[
            "Swap Views\tF7",
            "-",
            "Tile Horizontally",
            "Tile Vertically",
            "Tab Windows",
            "-",
            "Select Next Tab\t\u{2303}\u{21E5}",
            "Select Previous Tab\t\u{2303}\u{21E7}\u{21E5}",
            "-",
            "+Untitled 1: Floor Plan View",
        ],
    );
}

fn help_menu(ui: &mut egui::Ui, out: &mut Vec<Action>) {
    inert(
        ui,
        &[
            "Launch Help\u{2026}",
            "View Tutorial Guide\u{2026}",
            "View Reference Manual\u{2026}",
            "Visit Chief Architect Website\u{2026}",
            "View Training Videos\u{2026}",
            "Download Program Updates\u{2026}",
            "ChiefTalk\u{2026}",
            "Technical Support\u{2026}",
            "Export Logs\u{2026}",
            "System Information\u{2026}",
            "-",
        ],
    );
    live(ui, "About Plan Studio", "", false, Action::ShowAbout, out);
}
