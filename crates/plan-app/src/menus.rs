//! Chief-style pull-down menu bar: File, Edit, Build, Terrain, Library, 3D,
//! CAD, Tools, View, Window, Help (docs/chief-x18-menus.md).
//!
//! Build, Terrain and CAD submenus are generated from the same flyout tables
//! as the toolbar ([`toolbar::flyout_menu`]). Items that are not built yet are
//! shown disabled so the target shape of the product is visible.

use crate::shell::layout_window::{self, LayoutCommand};
use crate::theme::{CanvasTheme, BRIGHTNESS_MAX, BRIGHTNESS_MIN};
use crate::toolbar::{self, Action, BarState, Dock, FileCommand, FramingCommand, ViewFlag};
use crate::tools::cad::CadMode;
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
    ui.menu_button("File", |ui| file_menu(ui, state, out));
    ui.menu_button("Edit", |ui| edit_menu(ui, state, out));
    ui.menu_button("Build", |ui| build_menu(ui, state, out));
    ui.menu_button("Terrain", |ui| terrain_menu(ui, state, out));
    ui.menu_button("Library", |ui| {
        live(
            ui,
            "Catalog Settings\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::app_info::PREFS_LIBRARY),
            out,
        );
        live(
            ui,
            "Library Browser",
            "\u{2318}L",
            state.dock == Some(Dock::Library),
            Action::ToggleDock(Dock::Library),
            out,
        );
        ui.separator();
        use crate::tools::library::user as ulib;
        live(
            ui,
            "Add Selection to Library",
            "",
            false,
            Action::Custom(ulib::ADD_SELECTION),
            out,
        );
        live(
            ui,
            "Add Active Material to Library",
            "",
            false,
            Action::Custom(ulib::ADD_MATERIAL),
            out,
        );
        // The import window lives in the Library Browser: open that first.
        if ui.button("Import 3D Model (OBJ, glTF)\u{2026}").clicked() {
            if state.dock != Some(Dock::Library) {
                out.push(Action::ToggleDock(Dock::Library));
            }
            out.push(Action::Custom(ulib::IMPORT_MODEL));
            ui.close_menu();
        }
        ui.separator();
        live(
            ui,
            "Export Library (Plan Studio only)\u{2026}",
            "",
            false,
            Action::Custom(ulib::EXPORT_LIBRARY),
            out,
        );
        live(
            ui,
            "Import Library\u{2026}",
            "",
            false,
            Action::Custom(ulib::IMPORT_LIBRARY),
            out,
        );
    });
    ui.menu_button("3D", |ui| three_d_menu(ui, state, out));
    ui.menu_button("CAD", |ui| cad_menu(ui, state, out));
    ui.menu_button("Tools", |ui| tools_menu(ui, state, out));
    ui.menu_button("Layout", |ui| layout_menu(ui, out));
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

/// File > Open Recent Documents: the plans last opened or saved, newest
/// first.
fn recent_menu(ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let files = crate::dialogs::app_info::recent_files();
    ui.add_enabled_ui(!files.is_empty(), |ui| {
        ui.menu_button("Open Recent Documents", |ui| {
            for (i, f) in files
                .iter()
                .enumerate()
                .take(crate::dialogs::app_info::MAX_RECENT)
            {
                let name = f.file_name().map_or_else(
                    || f.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                live(
                    ui,
                    &name,
                    "",
                    false,
                    Action::Custom(crate::dialogs::app_info::RECENT[i]),
                    out,
                );
            }
            ui.separator();
            live(
                ui,
                "Clear Menu",
                "",
                false,
                Action::Custom(crate::files::CLEAR_RECENT),
                out,
            );
        });
    });
}

fn file_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
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
    ui.separator();
    live(
        ui,
        "Open Plan\u{2026}",
        "\u{2318}O",
        false,
        Action::FileOpen,
        out,
    );
    live(
        ui,
        "Open Layout\u{2026}",
        "",
        false,
        Action::Layout(LayoutCommand::ShowLayout),
        out,
    );
    recent_menu(ui, out);
    ui.separator();
    live(
        ui,
        "Close View",
        "",
        false,
        Action::Layout(LayoutCommand::ShowPlan),
        out,
    );
    live(
        ui,
        "Close Plan",
        "",
        false,
        Action::Custom(crate::files::CLOSE),
        out,
    );
    ui.separator();
    live(ui, "Save", "\u{2318}S", false, Action::FileSave, out);
    live(ui, "Save As\u{2026}", "", false, Action::FileSaveAs, out);
    live(
        ui,
        "Save a Copy\u{2026}",
        "",
        false,
        Action::Custom(crate::files::SAVE_COPY),
        out,
    );
    live(
        ui,
        "Revert to Saved",
        "",
        false,
        Action::Custom(crate::files::REVERT),
        out,
    );
    live(
        ui,
        "Backup Entire Plan\u{2026}",
        "",
        false,
        Action::Custom(crate::files::BACKUP),
        out,
    );
    live(
        ui,
        "Manage Auto Archives\u{2026}",
        "",
        false,
        Action::Custom(crate::files::ARCHIVES),
        out,
    );
    live(
        ui,
        "Show in Project Browser",
        "",
        false,
        Action::ToggleDock(Dock::Project),
        out,
    );
    live(
        ui,
        "View File Information\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::app_info::FILE_INFO),
        out,
    );
    ui.separator();
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
        live(
            ui,
            "Layout (JSON)\u{2026}",
            "",
            false,
            Action::File(FileCommand::ExportLayout),
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
        live(
            ui,
            "Chief Plan\u{2026}",
            "",
            false,
            Action::ImportChiefPlan,
            out,
        );
        live(
            ui,
            "Layout (JSON)\u{2026}",
            "",
            false,
            Action::File(FileCommand::ImportLayout),
            out,
        );
        live(
            ui,
            "Underlay Picture (PNG, JPEG, PDF)\u{2026}",
            "",
            false,
            Action::Custom(crate::tools::underlay::IMPORT),
            out,
        );
    });
    ui.menu_button("Print", |ui| {
        live(
            ui,
            "Print\u{2026}",
            "",
            false,
            Action::Layout(LayoutCommand::PrintDialog),
            out,
        );
        live(
            ui,
            "Print Preview",
            "",
            state.flags.contains(&ViewFlag::PrintPreview),
            Action::ToggleFlag(ViewFlag::PrintPreview),
            out,
        );
        live(
            ui,
            "Drawing Sheet Setup\u{2026}",
            "",
            false,
            Action::Layout(LayoutCommand::PageSetup),
            out,
        );
        live(
            ui,
            "Print Image\u{2026}",
            "",
            false,
            Action::Layout(LayoutCommand::PrintImage),
            out,
        );
        live(
            ui,
            "Print Model\u{2026}",
            "",
            false,
            Action::Layout(LayoutCommand::PrintModel),
            out,
        );
        ui.separator();
        live(
            ui,
            "Print Layout\u{2026}",
            "",
            false,
            Action::Layout(LayoutCommand::Print),
            out,
        );
        live(
            ui,
            "Export Layout PDF\u{2026}",
            "",
            false,
            Action::Layout(LayoutCommand::ExportPdf),
            out,
        );
    });
    ui.separator();
    live(
        ui,
        "Send to Layout\u{2026}",
        "S, L",
        false,
        Action::Layout(LayoutCommand::SendToLayout),
        out,
    );
    ui.separator();
    live(ui, "Quit", "", false, Action::Quit, out);
}

fn edit_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    undo_row(ui, "Undo", state.undo_label, "\u{2318}Z", Action::Undo, out);
    undo_row(ui, "Redo", state.redo_label, "\u{2318}Y", Action::Redo, out);
    ui.separator();
    edit_clipboard_rows(ui, state, out);
    live(
        ui,
        "Select Objects",
        "Space",
        state.tool == ToolId::Select,
        Action::SetTool(ToolId::Select),
        out,
    );
    edit_select_rows(ui, state, out);
    ui.separator();
    live(
        ui,
        "Snap Settings\u{2026}",
        "",
        false,
        Action::SnapSettings,
        out,
    );
    live(
        ui,
        "Edit Behaviors\u{2026}",
        "",
        false,
        Action::EditBehaviors,
        out,
    );
    inert(
        ui,
        &["Arc Creation Modes>", "-", "Edit Area>", "Stretch CAD", "-"],
    );
    live(
        ui,
        "Find/Replace Text\u{2026}",
        "",
        false,
        Action::FindReplaceText,
        out,
    );
    inert(ui, &["Replace Fonts\u{2026}", "-"]);
    live(
        ui,
        "Default Settings\u{2026}",
        "",
        false,
        Action::DefaultSettings,
        out,
    );
    live(
        ui,
        "Cabinet Defaults\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::defaults::CABINETS),
        out,
    );
    live(
        ui,
        "Preferences\u{2026}",
        "\u{2318},",
        false,
        Action::Custom(crate::dialogs::preferences::OPEN),
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

/// A live Edit row that runs the `edit.*` command `id`; its shortcut is the
/// live hotkey of the command named `name`.
fn edit_row(
    ui: &mut egui::Ui,
    state: &BarState,
    label: &str,
    name: &str,
    id: &'static str,
    out: &mut Vec<Action>,
) {
    let hk = state.hotkey(name, "");
    live(ui, label, &hk, false, Action::Custom(id), out);
}

/// Cut, Copy, Paste, Duplicate, Delete (S-81..S-84, S-88).
fn edit_clipboard_rows(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    use crate::editor::edit_commands::ids;
    edit_row(ui, state, "Cut", "Cut", ids::CUT, out);
    edit_row(ui, state, "Copy", "Copy", ids::COPY, out);
    edit_row(
        ui,
        state,
        "Copy and Paste in Place",
        "Copy and Paste in Place",
        ids::COPY_PASTE_IN_PLACE,
        out,
    );
    ui.menu_button("Paste", |ui| {
        edit_row(ui, state, "Paste", "Paste", ids::PASTE, out);
        edit_row(
            ui,
            state,
            "Paste Hold Position",
            "Paste Hold Position",
            ids::PASTE_HOLD,
            out,
        );
        ui.menu_button("Paste Special", |ui| {
            edit_row(
                ui,
                state,
                "As Group",
                "Paste as Group",
                ids::PASTE_GROUP,
                out,
            );
            edit_row(
                ui,
                state,
                "On Current Floor",
                "Paste Hold Position",
                ids::PASTE_HOLD,
                out,
            );
        });
    });
    edit_row(ui, state, "Duplicate", "Duplicate", ids::DUPLICATE, out);
    ui.separator();
    live(
        ui,
        "Delete",
        "\u{2326}",
        false,
        Action::Custom(ids::DELETE),
        out,
    );
    edit_row(
        ui,
        state,
        "Delete Objects\u{2026}",
        "Delete Objects",
        ids::DELETE_OBJECTS,
        out,
    );
    ui.separator();
}

/// Select All and Same Type, Group, the Transform/Replicate family, Align
/// and Distribute, Lock, Layer and the Action History.
fn edit_select_rows(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    use crate::editor::edit_commands::ids;
    use plan_core::transform::AlignMode;
    edit_row(ui, state, "Select All", "Select All", ids::SELECT_ALL, out);
    edit_row(
        ui,
        state,
        "Select Same Type",
        "Select Same Type",
        ids::SELECT_SAME,
        out,
    );
    edit_row(ui, state, "Group", "Group", ids::GROUP, out);
    edit_row(ui, state, "Ungroup", "Ungroup", ids::UNGROUP, out);
    ui.separator();
    edit_row(
        ui,
        state,
        "Transform/Replicate Object\u{2026}",
        "Transform/Replicate Object",
        ids::TRANSFORM,
        out,
    );
    edit_row(
        ui,
        state,
        "Rotate\u{2026}",
        "Rotate Selection",
        ids::ROTATE,
        out,
    );
    ui.menu_button("Reflect About Object", |ui| {
        edit_row(ui, state, "Move", "Reflect About Object", ids::REFLECT, out);
        edit_row(
            ui,
            state,
            "Copy",
            "Reflect Copy About Object",
            ids::REFLECT_COPY,
            out,
        );
    });
    edit_row(
        ui,
        state,
        "Point to Point Move",
        "Point to Point Move",
        ids::POINT_TO_POINT,
        out,
    );
    edit_row(
        ui,
        state,
        "Center Object",
        "Center Object",
        ids::CENTER,
        out,
    );
    ui.menu_button("Align", |ui| {
        for m in AlignMode::ALL {
            edit_row(ui, state, m.label(), m.label(), m.id(), out);
        }
        ui.separator();
        edit_row(
            ui,
            state,
            "Align/Distribute\u{2026}",
            "Align/Distribute",
            ids::ALIGN_DIALOG,
            out,
        );
    });
    ui.menu_button("Distribute", |ui| {
        edit_row(
            ui,
            state,
            "Distribute Horizontally",
            "Distribute Horizontally",
            ids::DISTRIBUTE_H,
            out,
        );
        edit_row(
            ui,
            state,
            "Distribute Vertically",
            "Distribute Vertically",
            ids::DISTRIBUTE_V,
            out,
        );
    });
    edit_row(
        ui,
        state,
        "Make Parallel",
        "Make Parallel",
        ids::PARALLEL,
        out,
    );
    edit_row(
        ui,
        state,
        "Make Perpendicular",
        "Make Perpendicular",
        ids::PERPENDICULAR,
        out,
    );
    edit_row(ui, state, "Move to Front", "Move to Front", ids::FRONT, out);
    edit_row(ui, state, "Move to Back", "Move to Back", ids::BACK, out);
    ui.separator();
    edit_row(ui, state, "Lock", "Lock Selection", ids::LOCK, out);
    edit_row(ui, state, "Unlock", "Unlock Selection", ids::UNLOCK, out);
    edit_row(
        ui,
        state,
        "Send to Layer\u{2026}",
        "Send to Layer",
        ids::LAYER,
        out,
    );
    edit_row(
        ui,
        state,
        "Action History",
        "Action History",
        ids::HISTORY,
        out,
    );
    ui.separator();
}

fn build_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    for group in toolbar::build_menu() {
        match group.name {
            Some(name) => {
                ui.menu_button(name, |ui| {
                    for f in &group.flyouts {
                        toolbar::flyout_menu(ui, f, state, out);
                    }
                    if name == "Framing" {
                        framing_rows(ui, state, out);
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

/// Build > Framing: the defaults window and the Framing Overview plan view.
fn framing_rows(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    ui.separator();
    live(
        ui,
        "Framing Defaults\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::defaults::FRAMING),
        out,
    );
    live(
        ui,
        "Framing Overview",
        "",
        state.view_name == crate::editor::framing_view::OVERVIEW,
        Action::Custom(crate::dialogs::defaults::FRAMING_OVERVIEW),
        out,
    );
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
            ("Auto Interior Elevations", V::AutoInterior),
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
    {
        use crate::tools::materials as mat;
        live(
            ui,
            "Materials\u{2026}",
            "",
            false,
            Action::Custom(mat::LIST),
            out,
        );
        live(
            ui,
            "Material Painter",
            "",
            mat::is_mode_active(mat::PAINTER),
            Action::Custom(mat::PAINTER),
            out,
        );
        live(
            ui,
            "Adjust Materials\u{2026}",
            "",
            false,
            Action::Custom(mat::ADJUST),
            out,
        );
        live(
            ui,
            "Material Builder\u{2026}",
            "",
            false,
            Action::Custom(mat::BUILDER),
            out,
        );
    }
    ui.separator();
    ui.menu_button("Lighting", |ui| {
        live(
            ui,
            "Add Lights",
            "",
            false,
            Action::SetTool(ToolId::CameraVariant(V::AddLights)),
            out,
        );
        live(ui, "Adjust Lights", "", false, cmd(C::AdjustLights), out);
    });
    toolbar::flyout_menu(ui, &toolbar::rendering_techniques(), state, out);
    live(
        ui,
        "Delete Surface",
        "",
        crate::tools::materials::is_mode_active(crate::tools::materials::ERASE),
        Action::Custom(crate::tools::materials::ERASE),
        out,
    );
    live(ui, "Rebuild 3D", "", false, cmd(C::Rebuild), out);
    live(
        ui,
        "Show Doors Open",
        "",
        false,
        Action::Custom(crate::editor::opening_edit::DOORS_OPEN),
        out,
    );
    live(
        ui,
        "Casing, Jambs and Sills",
        "",
        false,
        Action::Custom(crate::editor::opening_edit::CASING_3D),
        out,
    );
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

/// A CAD menu entry that starts the CAD tool in mode `m`.
fn cad_mode(ui: &mut egui::Ui, label: &str, m: CadMode, out: &mut Vec<Action>) {
    live(
        ui,
        label,
        "",
        false,
        Action::SetTool(ToolId::CadVariant(m)),
        out,
    );
}

fn cad_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    for f in [
        toolbar::points(),
        toolbar::lines(),
        toolbar::arcs(),
        toolbar::circles(),
        toolbar::boxes(),
    ] {
        toolbar::flyout_menu(ui, &f, state, out);
    }
    cad_mode(ui, "Revision Cloud", CadMode::RevisionCloud, out);
    cad_mode(ui, "Spline", CadMode::Spline, out);
    ui.separator();
    toolbar::flyout_menu(ui, &toolbar::dimensions(), state, out);
    toolbar::flyout_menu(ui, &toolbar::auto_dimensions(), state, out);
    ui.separator();
    toolbar::flyout_menu(ui, &toolbar::text_tools(), state, out);
    ui.menu_button("Patterns", |ui| {
        cad_mode(ui, "Hatch Closed Shape", CadMode::Hatch, out);
    });
    toolbar::flyout_menu(ui, &toolbar::cad_blocks(), state, out);
    ui.menu_button("Edit CAD", |ui| {
        for m in [
            CadMode::Fillet,
            CadMode::Chamfer,
            CadMode::Offset,
            CadMode::Trim,
            CadMode::Extend,
            CadMode::BreakLine,
            CadMode::ReverseDirection,
            CadMode::MakeParallel,
            CadMode::MakePerpendicular,
        ] {
            cad_mode(ui, m.name(), m, out);
        }
        ui.separator();
        for m in [
            CadMode::ConvertToPolyline,
            CadMode::ConvertToSpline,
            CadMode::PolylineToLines,
        ] {
            cad_mode(ui, m.name(), m, out);
        }
    });
    ui.separator();
    live(
        ui,
        "Sun Angle",
        "",
        state.flags.contains(&ViewFlag::SunAngle),
        Action::ToggleFlag(ViewFlag::SunAngle),
        out,
    );
    ui.separator();
    live(ui, "Plan Footprint", "", false, Action::PlanFootprint, out);
    ui.separator();
    cad_mode(
        ui,
        "CAD Block Management\u{2026}",
        CadMode::BlockManagement,
        out,
    );
    cad_mode(ui, "CAD Detail From View", CadMode::DetailFromView, out);
    ui.separator();
    live(
        ui,
        "CAD to Walls\u{2026}",
        "",
        false,
        Action::File(FileCommand::CadToWalls),
        out,
    );
}

fn tools_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    ui.menu_button("Layer Settings", |ui| {
        live(
            ui,
            "Display Options\u{2026}",
            "",
            false,
            Action::OpenLayerDisplay,
            out,
        );
        live(
            ui,
            "Layer Set Management\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::layer_sets::OPEN),
            out,
        );
        live(
            ui,
            "Active Layers by Tool\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::layer_sets::ACTIVE_LAYERS),
            out,
        );
    });
    live(
        ui,
        "Floor/Reference Display\u{2026}",
        "",
        false,
        Action::ReferenceDisplayOptions,
        out,
    );
    live(
        ui,
        "Underlays\u{2026}",
        "",
        false,
        Action::Custom(crate::tools::underlay::MANAGE),
        out,
    );
    ui.menu_button("Active View", |ui| {
        for (i, v) in state.views.iter().enumerate() {
            live(
                ui,
                &v.name,
                "",
                v.name == state.view_name,
                Action::PlanView(i),
                out,
            );
        }
    });
    ui.menu_button("Plan Views", |ui| {
        live(
            ui,
            "Plan View Specification\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::plan_views::OPEN),
            out,
        );
        live(
            ui,
            "Save Plan View",
            "",
            false,
            Action::Custom(crate::dialogs::plan_views::SAVE),
            out,
        );
        live(
            ui,
            "Reset Plan View",
            "",
            false,
            Action::Custom(crate::dialogs::plan_views::RESET),
            out,
        );
        ui.separator();
        live(
            ui,
            "Add Template Plan Views",
            "",
            false,
            Action::Custom(crate::dialogs::plan_views::SEED),
            out,
        );
    });
    live(
        ui,
        "Active Defaults\u{2026}",
        "",
        false,
        Action::DefaultSettings,
        out,
    );
    ui.separator();
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
            "Customize Toolbars\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::app_info::CUSTOMIZE_TOOLBARS),
            out,
        );
        live(
            ui,
            "Customize Hotkeys\u{2026}",
            "",
            false,
            Action::OpenHotkeyDialog,
            out,
        );
    });
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
        // Click in the plan to place the schedule as a table that stays up
        // to date (the Schedule flyout's tool).
        ui.menu_button("Place on Plan", |ui| {
            for k in crate::tools::schedule::FLYOUT_KINDS {
                live(
                    ui,
                    crate::tools::schedule::entry_name(k),
                    "",
                    false,
                    Action::SetTool(ToolId::ScheduleVariant(k)),
                    out,
                );
            }
        });
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
    live(
        ui,
        "Project Information\u{2026}",
        "",
        false,
        Action::ProjectInfo,
        out,
    );
    ui.separator();
    live(
        ui,
        "Color Chooser\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::app_info::COLOR_CHOOSER),
        out,
    );
    ui.separator();
    live(
        ui,
        "New Plan View",
        "",
        false,
        Action::Custom(crate::dialogs::app_info::NEW_PLAN_VIEW),
        out,
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
    live(
        ui,
        "Refresh Display",
        "F5",
        false,
        Action::Custom(crate::dialogs::app_info::REFRESH),
        out,
    );
    ui.separator();
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
    live(
        ui,
        "Active Layer Display Options",
        "",
        dock(Dock::LayerDisplay),
        Action::ToggleDock(Dock::LayerDisplay),
        out,
    );
    live(
        ui,
        "Walkthrough Preview",
        "",
        false,
        Action::View3d(crate::shell::view3d_panel::View3dCommand::PlayWalkthrough),
        out,
    );
    edit_row(
        ui,
        state,
        "Action History",
        "Action History",
        crate::editor::edit_commands::ids::HISTORY,
        out,
    );
    ui.separator();
    live(
        ui,
        "Status Bar",
        "",
        crate::dialogs::preferences::show_status_bar(),
        Action::Custom(crate::dialogs::app_info::TOGGLE_STATUS_BAR),
        out,
    );
    live(
        ui,
        "Toolbars",
        "",
        crate::dialogs::preferences::show_toolbars(),
        Action::Custom(crate::dialogs::app_info::TOGGLE_TOOLBARS),
        out,
    );
    ui.separator();
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
    ui.separator();
    live(
        ui,
        "Reference Grid",
        "\u{21E7}F9",
        flag(ViewFlag::ReferenceGrid),
        Action::ToggleFlag(ViewFlag::ReferenceGrid),
        out,
    );
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
    ui.separator();
    live(
        ui,
        "Enter Full Screen",
        "",
        false,
        Action::Custom(crate::dialogs::app_info::FULL_SCREEN),
        out,
    );
    ui.separator();

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
    live(
        ui,
        "Fill Window Selected Objects",
        "",
        false,
        Action::Custom(crate::dialogs::app_info::FILL_SELECTED),
        out,
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
    ui.separator();
    let in_layout = layout_window::is_active();
    live(
        ui,
        "Floor Plan View",
        "",
        !in_layout,
        Action::Layout(LayoutCommand::ShowPlan),
        out,
    );
    live(
        ui,
        "Layout",
        "",
        in_layout,
        Action::Layout(LayoutCommand::ShowLayout),
        out,
    );
}

/// The Layout menu: Chief's layout commands (page management, the page
/// table, box specification) plus Page Setup and Project Information.
fn layout_menu(ui: &mut egui::Ui, out: &mut Vec<Action>) {
    use LayoutCommand as C;
    let row = |ui: &mut egui::Ui, label: &str, c: C, out: &mut Vec<Action>| {
        live(ui, label, "", false, Action::Layout(c), out);
    };
    row(ui, "Send to Layout\u{2026}", C::SendToLayout, out);
    row(ui, "Send All Floors to Layout", C::SendAllFloors, out);
    ui.separator();
    row(
        ui,
        "Layout Box Specification\u{2026}",
        C::BoxSpecification,
        out,
    );
    row(ui, "Delete Layout Box", C::DeleteBox, out);
    row(ui, "Update Layout Views", C::UpdateViews, out);
    ui.separator();
    row(ui, "Insert Page Before", C::InsertPageBefore, out);
    row(ui, "Insert Page After", C::InsertPageAfter, out);
    row(ui, "Duplicate Page", C::DuplicatePage, out);
    row(ui, "Delete Page", C::DeletePage, out);
    row(
        ui,
        "Exchange With Previous Page",
        C::ExchangeWithPrevious,
        out,
    );
    row(ui, "Exchange With Next Page", C::ExchangeWithNext, out);
    ui.separator();
    row(ui, "Previous Page", C::PreviousPage, out);
    row(ui, "Next Page", C::NextPage, out);
    row(ui, "Layout Page Table\u{2026}", C::PageTable, out);
    ui.separator();
    row(ui, "Page Setup\u{2026}", C::PageSetup, out);
    row(ui, "Project Information\u{2026}", C::ProjectInfo, out);
    row(ui, "Fit Page in Window", C::FitPage, out);
    row(ui, "Layer Display Options\u{2026}", C::LayerDisplay, out);
    row(ui, "Add Sheet Index", C::AddSheetIndex, out);
    ui.separator();
    row(ui, "Save As Template\u{2026}", C::SaveAsTemplate, out);
    row(ui, "Apply Template\u{2026}", C::ApplyTemplate, out);
    ui.separator();
    row(ui, "Print Model\u{2026}", C::PrintModel, out);
    row(ui, "Print Layout\u{2026}", C::Print, out);
    row(ui, "Export Layout PDF\u{2026}", C::ExportPdf, out);
}

fn help_menu(ui: &mut egui::Ui, out: &mut Vec<Action>) {
    use crate::dialogs::app_info as app;
    for (label, id) in [
        ("Launch Help\u{2026}", app::HELP),
        ("View Tutorial Guide\u{2026}", app::HELP_TUTORIAL),
        ("View Reference Manual\u{2026}", app::HELP_REFERENCE),
        ("Keyboard Shortcuts\u{2026}", app::HELP_HOTKEYS),
        ("Plan Studio on GitHub\u{2026}", app::HELP_PROJECT),
    ] {
        live(ui, label, "", false, Action::Custom(id), out);
    }
    ui.separator();
    live(
        ui,
        "System Information\u{2026}",
        "",
        false,
        Action::Custom(app::SYSTEM_INFO),
        out,
    );
    ui.separator();
    live(
        ui,
        "About Plan Studio",
        "",
        false,
        Action::Custom(app::ABOUT),
        out,
    );
}

#[cfg(test)]
mod tests {
    /// Every menu row outside the Edit menu is live. (The Edit menu's last
    /// dimmed rows belong to the editing commands, not to this check.)
    #[test]
    fn no_dimmed_rows_remain_outside_the_edit_menu() {
        let src = include_str!("menus.rs");
        let body = &src[..src.find("#[cfg(test)]").unwrap_or(src.len())];
        let mut current = "";
        let mut dimmed: Vec<String> = Vec::new();
        for line in body.lines() {
            if let Some(rest) = line.strip_prefix("fn ") {
                current = rest.split('(').next().unwrap_or("");
            }
            if line.contains("inert(") && !line.starts_with("fn inert") && current != "edit_menu" {
                dimmed.push(format!("{current}: {}", line.trim()));
            }
        }
        assert!(dimmed.is_empty(), "dimmed rows outside Edit: {dimmed:?}");
    }

    /// Every command id the menus and toolbars send is handled.
    #[test]
    fn the_commands_of_the_menus_are_handled() {
        use crate::dialogs::{app_info as app, preferences};
        use crate::editor::{placed, EditorContext};
        use crate::tools::{materials as mat, underlay};
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        for id in [
            underlay::MANAGE,
            mat::LIST,
            mat::ADJUST,
            mat::BUILDER,
            mat::PAINTER,
            mat::EYEDROPPER,
            mat::ERASE,
            preferences::OPEN,
            app::REFRESH,
            app::FULL_SCREEN,
            app::FILE_INFO,
            app::COLOR_CHOOSER,
            app::SYSTEM_INFO,
            app::NEW_PLAN_VIEW,
            app::TOGGLE_STATUS_BAR,
            app::TOGGLE_TOOLBARS,
            app::FILL_SELECTED,
            app::PREFS_LIBRARY,
            app::HELP,
            app::HELP_TUTORIAL,
            app::HELP_REFERENCE,
            app::HELP_HOTKEYS,
            app::ABOUT,
            app::CUSTOMIZE_TOOLBARS,
            app::RECENT[0],
            crate::dialogs::defaults::CABINETS,
            crate::dialogs::defaults::FRAMING,
        ] {
            assert!(placed::run_command(&mut cx, id), "{id} is not handled");
        }
        mat::set_painter_mode(mat::PainterMode::Off);
    }
}
