//! Chief-style pull-down menu bar: File, Edit, Build, Terrain, Library, 3D,
//! CAD, Tools, View, Window, Help (docs/chief-x18-menus.md).
//!
//! Build, Terrain and CAD submenus are generated from the same flyout tables
//! as the toolbar ([`toolbar::flyout_menu`]). Items that are not built yet are
//! shown disabled so the target shape of the product is visible.

use crate::theme::{CanvasTheme, BRIGHTNESS_MAX, BRIGHTNESS_MIN};
use crate::toolbar::{self, Action, BarState, Dock, FileCommand, FramingCommand, ViewFlag};
use crate::tools::ToolId;
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
    ui.menu_button("3D", |ui| three_d_menu(ui, state, out));
    ui.menu_button("CAD", |ui| cad_menu(ui, state, out));
    ui.menu_button("Tools", |ui| tools_menu(ui, out));
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

/// Edit > Undo / Redo: named after the step (`Undo Move Wall`), disabled when
/// there is none.
fn undo_row(
    ui: &mut egui::Ui,
    verb: &str,
    label: Option<&str>,
    shortcut: &str,
    action: Action,
    out: &mut Vec<Action>,
) {
    let text = match label {
        Some(l) => format!("{verb} {l}"),
        None => verb.to_string(),
    };
    let btn = egui::Button::new(text).shortcut_text(toolbar::pretty_hotkey(shortcut));
    if ui.add_enabled(label.is_some(), btn).clicked() {
        out.push(action);
        ui.close_menu();
    }
}

fn file_menu(ui: &mut egui::Ui, out: &mut Vec<Action>) {
    live(ui, "New Plan", "\u{2318}N", false, Action::FileNew, out);
    live(ui, "New Layout", "", false, Action::FileNewLayout, out);
    ui.menu_button("Templates", |ui| {
        live(
            ui,
            "Save Current Defaults as My Template\u{2026}",
            "",
            false,
            Action::SaveTemplate,
            out,
        );
        live(
            ui,
            "Import Chief Template\u{2026}",
            "",
            false,
            Action::ImportChiefTemplate,
            out,
        );
        live(
            ui,
            "Reset to Chief X18 Template",
            "",
            false,
            Action::ResetTemplate,
            out,
        );
    });
    inert(ui, &["-"]);
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
        ],
    );
    ui.menu_button("Export", |ui| {
        use crate::shell::view3d_panel::View3dCommand;
        live(
            ui,
            "DXF\u{2026}",
            "",
            false,
            Action::File(FileCommand::ExportDxf),
            out,
        );
        live(
            ui,
            "Elevation DXF\u{2026}",
            "",
            false,
            Action::File(FileCommand::ExportElevationsDxf),
            out,
        );
        live(
            ui,
            "Construction Set PDF\u{2026}",
            "",
            false,
            Action::CreateConstructionSet,
            out,
        );
        live(
            ui,
            "glTF\u{2026}",
            "",
            false,
            Action::View3d(View3dCommand::ExportGltf),
            out,
        );
    });
    ui.menu_button("Import", |ui| {
        live(
            ui,
            "Import Drawing (DXF)\u{2026}",
            "",
            false,
            Action::File(FileCommand::ImportDxf),
            out,
        );
    });
    inert(ui, &["Print>", "-", "Send to Layout\u{2026}\tS, L", "-"]);
    live(ui, "Quit", "", false, Action::Quit, out);
}

fn edit_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    undo_row(ui, "Undo", state.undo_label, "\u{2318}Z", Action::Undo, out);
    undo_row(ui, "Redo", state.redo_label, "\u{2318}Y", Action::Redo, out);
    inert(
        ui,
        &[
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
        state.tool == ToolId::Select,
        Action::SetTool(ToolId::Select),
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
        ],
    );
    live(
        ui,
        "Default Settings\u{2026}",
        "",
        false,
        Action::DefaultSettings,
        out,
    );
    inert(
        ui,
        &[
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
    use crate::tools::terrain::TerrainVariant as T;
    use toolbar::TerrainCommand as C;
    live(
        ui,
        "Create Terrain Perimeter",
        "",
        false,
        Action::SetTool(ToolId::TerrainVariant(T::Perimeter)),
        out,
    );
    ui.separator();
    live(
        ui,
        "Terrain Specification\u{2026}",
        "",
        false,
        Action::Terrain(C::Specification),
        out,
    );
    live(
        ui,
        "Build Terrain",
        "",
        false,
        Action::SetTool(ToolId::TerrainVariant(T::Build)),
        out,
    );
    live(
        ui,
        "Clear Terrain",
        "",
        false,
        Action::Terrain(C::Clear),
        out,
    );
    live(
        ui,
        "Make Terrain Hole Around Building",
        "",
        false,
        Action::Terrain(C::HoleAroundBuilding),
        out,
    );
    ui.separator();
    for f in &toolbar::terrain_menu() {
        toolbar::flyout_menu(ui, f, state, out);
    }
}

fn three_d_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    use crate::shell::view3d_panel::View3dCommand as C;
    use crate::tools::camera::CameraVariant as V;
    use plan_view3d::CameraMode as M;
    let cmd = |c| Action::View3d(c);
    ui.menu_button("Create Orthographic View", |ui| {
        for (name, mode) in [
            ("Front Elevation", M::ElevationFront),
            ("Back Elevation", M::ElevationBack),
            ("Left Elevation", M::ElevationLeft),
            ("Right Elevation", M::ElevationRight),
            ("Plan Overhead", M::PlanOverhead),
        ] {
            live(ui, name, "", false, cmd(C::Mode(mode)), out);
        }
        ui.separator();
        live(
            ui,
            V::CrossSection.label(),
            "",
            false,
            cmd(C::Tool(V::CrossSection)),
            out,
        );
        live(
            ui,
            V::BackClippedSection.label(),
            "",
            false,
            cmd(C::Tool(V::BackClippedSection)),
            out,
        );
        ui.separator();
        live(
            ui,
            "Export Elevations (DXF)\u{2026}",
            "",
            false,
            Action::File(FileCommand::ExportElevationsDxf),
            out,
        );
    });
    ui.menu_button("Create Perspective View", |ui| {
        live(
            ui,
            V::FullCamera.label(),
            "\u{21E7}J",
            false,
            cmd(C::Tool(V::FullCamera)),
            out,
        );
        live(
            ui,
            V::FullOverview.label(),
            "\u{21E7}K",
            false,
            cmd(C::Mode(M::Orbit)),
            out,
        );
        live(
            ui,
            V::FloorOverview.label(),
            "",
            false,
            cmd(C::FloorOverview),
            out,
        );
        live(
            ui,
            V::DollHouse.label(),
            "",
            false,
            cmd(C::Mode(M::DollHouse)),
            out,
        );
        ui.separator();
        live(ui, "Ray Trace\u{2026}", "", false, cmd(C::RayTrace), out);
    });
    ui.menu_button("Create Auto Elevations", |ui| {
        for (name, v) in [
            ("Auto Elevations", V::AutoElevation),
            ("Auto Back-Clipped Elevations", V::AutoBackclipped),
            ("Wall Elevation Camera", V::WallElevation),
        ] {
            live(
                ui,
                name,
                "",
                false,
                Action::SetTool(ToolId::CameraVariant(v)),
                out,
            );
        }
    });
    ui.separator();
    inert(
        ui,
        &[
            "Move Camera with Mouse>",
            "Move Camera with Keyboard>",
            "Move Camera>",
            "Orbit Camera>",
            "Tilt Camera>",
            "View Direction>",
            "Isometric Views>",
        ],
    );
    ui.separator();
    ui.menu_button("Walkthroughs", |ui| {
        live(
            ui,
            "Create Walkthrough Path",
            "",
            false,
            Action::SetTool(ToolId::CameraVariant(V::Walkthrough)),
            out,
        );
        live(
            ui,
            "Play Walkthrough",
            "",
            false,
            cmd(C::PlayWalkthrough),
            out,
        );
        live(
            ui,
            "Record Walkthrough\u{2026}",
            "",
            false,
            cmd(C::RecordWalkthrough),
            out,
        );
    });
    ui.separator();
    inert(
        ui,
        &[
            "Materials>",
            "Material Painter>",
            "Adjust Materials>",
            "Adjust 3D Cladding>",
            "Material Builder\u{2026}",
            "-",
            "Lighting>",
            "Camera View Options>",
        ],
    );
    toolbar::flyout_menu(ui, &toolbar::rendering_techniques(), state, out);
    inert(ui, &["Toggle Patterns", "Delete Surface"]);
    live(ui, "Rebuild 3D", "", false, cmd(C::Rebuild), out);
    ui.menu_button("Export", |ui| {
        live(ui, "glTF\u{2026}", "", false, cmd(C::ExportGltf), out);
    });
    ui.separator();
    live(
        ui,
        "3D View Defaults\u{2026}",
        &state.hotkey("3D View Defaults", "\u{2318}1"),
        false,
        cmd(C::Defaults),
        out,
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
        ],
    );
    live(
        ui,
        "CAD to Walls\u{2026}",
        "",
        false,
        Action::File(FileCommand::CadToWalls),
        out,
    );
}

fn tools_menu(ui: &mut egui::Ui, out: &mut Vec<Action>) {
    ui.menu_button("Layer Settings", |ui| {
        live(
            ui,
            "Display Options\u{2026}",
            "",
            false,
            Action::OpenLayerDisplay,
            out,
        );
    });
    inert(
        ui,
        &[
            "Floor/Reference Display>",
            "Active View>",
            "Active Defaults\u{2026}",
            "-",
        ],
    );
    ui.menu_button("Checks", |ui| {
        live(ui, "Plan Check", "", false, Action::PlanCheck, out);
        live(
            ui,
            "Door/Window Check",
            "",
            false,
            Action::DoorWindowCheck,
            out,
        );
        live(ui, "Plan Footprint", "", false, Action::PlanFootprint, out);
    });
    ui.menu_button("Toolbars and Hotkeys", |ui| {
        live(
            ui,
            "Customize Hotkeys\u{2026}",
            "",
            false,
            Action::OpenHotkeyDialog,
            out,
        );
    });
    inert(ui, &["Symbol>"]);
    ui.menu_button("Space Planning", |ui| {
        live(
            ui,
            "Space Planning Assistant\u{2026}",
            "",
            false,
            Action::SpacePlanning,
            out,
        );
    });
    inert(ui, &["Plan Database>", "Time Tracker>"]);
    ui.menu_button("Schedules", |ui| {
        live(ui, "Door Schedule", "", false, Action::DoorSchedule, out);
        live(
            ui,
            "Window Schedule",
            "",
            false,
            Action::WindowSchedule,
            out,
        );
        live(ui, "Room Schedule", "", false, Action::RoomSchedule, out);
        live(ui, "Wall Schedule", "", false, Action::WallSchedule, out);
        ui.separator();
        live(
            ui,
            "Create Construction Set\u{2026}",
            "",
            false,
            Action::CreateConstructionSet,
            out,
        );
        ui.separator();
        live(
            ui,
            "Framing Takeoff\u{2026}",
            "",
            false,
            Action::Framing(FramingCommand::Takeoff),
            out,
        );
    });
    live(
        ui,
        "Materials List\u{2026}",
        "",
        false,
        Action::MaterialsList,
        out,
    );
    inert(
        ui,
        &[
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
    // Daniel's "-" is Zoom In; the labels follow the live hotkey map.
    let key = |name: &str| state.hotkey(name, "");
    live(
        ui,
        "Zoom Out",
        &key("Zoom Out"),
        false,
        Action::ZoomOut,
        out,
    );
    live(ui, "Zoom In", &key("Zoom In"), false, Action::ZoomIn, out);
    live(
        ui,
        "Undo Zoom",
        &key("Undo Zoom"),
        false,
        Action::UndoZoom,
        out,
    );
    inert(
        ui,
        &["Fill Window Building Only", "Fill Window Selected Objects"],
    );
    live(
        ui,
        "Fill Window",
        &state.hotkey("Fill Window", "\u{2303}F"),
        false,
        Action::FillWindow,
        out,
    );
    live(
        ui,
        "Pan Window",
        &state.hotkey("Pan Window", "H"),
        state.tool == ToolId::Pan,
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
