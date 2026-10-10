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

/// Edit > Replace Fonts: opens the Text Styles list, whose Replace Fonts section
/// does the work (Chief TXT-12).
pub const REPLACE_FONTS: &str = "edit.replace_fonts";

/// Draws the menu bar contents; actions are appended to `out`.
pub fn bar(
    ui: &mut egui::Ui,
    state: &BarState,
    theme: CanvasTheme,
    brightness: &mut f32,
    out: &mut Vec<Action>,
) {
    // Windows the menu commands open (Replace Fonts, Export to REScheck).
    crate::dialogs::find_replace::show_prompts(ui.ctx(), out);
    crate::dialogs::text::rescheck::show_windows(ui.ctx(), out);
    crate::dialogs::wall_types::show_windows(ui.ctx(), out);
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
        live(
            ui,
            "Convert to Symbol",
            "",
            false,
            Action::Custom(crate::tools::library::convert::CONVERT_TO_SYMBOL),
            out,
        );
        live(
            ui,
            "Replace From Library",
            "",
            false,
            Action::Custom(crate::tools::library::convert::REPLACE_FROM_LIBRARY),
            out,
        );
        ui.menu_button("Moldings", |ui| {
            use crate::tools::molding as mold;
            for (label, id) in [
                ("Add Closed Polyline as Molding Profile", mold::ADD_PROFILE),
                (
                    "Add Closed Polylines as Stacked Molding",
                    mold::ADD_STACKED_PROFILE,
                ),
                ("Place Molding Profile", mold::PLACE_PROFILE),
                ("Edit Molding Profile", mold::EDIT_PROFILE),
                ("-", ""),
                ("Replace Moldings From Library", mold::REPLACE_FROM_LIBRARY),
                ("-", ""),
                ("Make Room Molding Polyline", mold::MAKE_ROOM_POLYLINE),
                (
                    "Make Exterior Room Molding Polyline",
                    mold::MAKE_EXTERIOR_POLYLINE,
                ),
                ("Make Cabinet Molding Polyline", mold::MAKE_CABINET_POLYLINE),
                ("Reverse Direction", mold::REVERSE_DIRECTION),
                ("Remove Molding from Selected Edge", mold::REMOVE_EDGE),
                ("Add Molding to Selected Edge", mold::ADD_EDGE),
                ("Select Next Edge", mold::NEXT_EDGE),
                ("-", ""),
                (
                    "Include Inside Corners for Auto Place Trim",
                    mold::INCLUDE_INSIDE,
                ),
            ] {
                if label == "-" {
                    ui.separator();
                } else {
                    live(ui, label, "", false, Action::Custom(id), out);
                }
            }
        });
        // The import window lives in the Library Browser: open that first.
        if ui
            .button("Import 3D Model (STL, 3DS, DAE, OBJ, glTF)\u{2026}")
            .clicked()
        {
            if state.dock != Some(Dock::Library) {
                out.push(Action::ToggleDock(Dock::Library));
            }
            out.push(Action::Custom(ulib::IMPORT_MODEL));
            ui.close_menu();
        }
        ui.separator();
        live(
            ui,
            "Export Library (Plan Studio JSON)\u{2026}",
            "",
            false,
            Action::Custom(ulib::EXPORT_LIBRARY),
            out,
        );
        live(
            ui,
            "Import Library (Plan Studio JSON, Chief .calib, .calibz)\u{2026}",
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
        ui.separator();
        for (label, id) in [
            (
                "New Plan from Template\u{2026}",
                crate::dialogs::template_chooser::NEW_PLAN,
            ),
            (
                "New Layout from Template\u{2026}",
                crate::dialogs::template_chooser::NEW_LAYOUT,
            ),
            (
                "Save as Template\u{2026}",
                crate::dialogs::template_chooser::SAVE_AS,
            ),
        ] {
            live(ui, label, "", false, Action::Custom(id), out);
        }
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
        Action::Custom(crate::shell::view_commands::CLOSE_VIEW),
        out,
    );
    live(
        ui,
        "Close All 3D Views",
        "",
        false,
        Action::Custom(crate::shell::view_commands::CLOSE_ALL_3D),
        out,
    );
    live(
        ui,
        "Close All Views",
        "",
        false,
        Action::Custom(crate::shell::view_commands::CLOSE_ALL_VIEWS),
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
        live(
            ui,
            "Thermal Envelope Data\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::text::rescheck::EXPORT_THERMAL),
            out,
        );
        live(
            ui,
            "Export to REScheck\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::text::rescheck::EXPORT_RESCHECK),
            out,
        );
        live(
            ui,
            "Picture (PNG, JPEG, BMP, TIFF)\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::export_picture::EXPORT_PICTURE),
            out,
        );
    });
    ui.menu_button("Import", |ui| {
        live(
            ui,
            "Import Settings from Plan/Layout\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::import_settings::IMPORT_SETTINGS),
            out,
        );
        ui.menu_button("Legacy Settings Import", |ui| {
            use crate::dialogs::import_settings as imp;
            for (label, id) in [
                ("Import Layer Sets\u{2026}", imp::LEGACY_LAYERS),
                ("Import Default Sets\u{2026}", imp::LEGACY_DEFAULT_SETS),
                ("Import Wall Definitions\u{2026}", imp::LEGACY_WALLS),
                ("Import Note Types\u{2026}", imp::LEGACY_NOTES),
            ] {
                live(ui, label, "", false, Action::Custom(id), out);
            }
        });
        ui.separator();
        live(
            ui,
            "Import Drawing (DWG/DXF)\u{2026}",
            "",
            false,
            Action::File(FileCommand::ImportDxf),
            out,
        );
        live(
            ui,
            "Terrain Data\u{2026}",
            "",
            false,
            Action::SetTool(ToolId::TerrainVariant(
                crate::tools::terrain::TerrainVariant::ImportData,
            )),
            out,
        );
        live(
            ui,
            "GPS Data\u{2026}",
            "",
            false,
            Action::SetTool(ToolId::TerrainVariant(
                crate::tools::terrain::TerrainVariant::ImportGps,
            )),
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
            "Line Styles (.lin)\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::line_style::IMPORT),
            out,
        );
        live(
            ui,
            "Patterns (.pat)\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::pattern_editor::IMPORT),
            out,
        );
        live(
            ui,
            "Picture (PNG, JPEG)\u{2026}",
            "",
            false,
            Action::Custom(crate::tools::images::IMPORT_PICTURE),
            out,
        );
        live(
            ui,
            "Material Package (Lightbeans zip)\u{2026}",
            "",
            false,
            Action::Custom(crate::tools::materials::package::IMPORT_PACKAGE),
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
        live(
            ui,
            "3D Symbol (STL, 3DS, DAE, OBJ, glTF)\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::export_picture::IMPORT_3D_SYMBOL),
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
        // A layout's sheet is its Page Setup; every other view has its own
        // Drawing Sheet Setup.
        live(
            ui,
            "Drawing Sheet Setup\u{2026}",
            "",
            false,
            if crate::shell::layout_window::is_active() {
                Action::Layout(LayoutCommand::PageSetup)
            } else {
                Action::Custom(crate::dialogs::drawing_sheet::OPEN)
            },
            out,
        );
        live(
            ui,
            "Scale to Fit",
            "",
            false,
            Action::Custom(crate::dialogs::drawing_sheet::SCALE_TO_FIT),
            out,
        );
        live(
            ui,
            "Center Sheet",
            "",
            false,
            Action::Custom(crate::dialogs::drawing_sheet::CENTER_SHEET),
            out,
        );
        live(
            ui,
            "Customize Sheet Sizes\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::drawing_sheet::CUSTOMIZE),
            out,
        );
        live(
            ui,
            "Clear Printer Info",
            "",
            false,
            Action::Custom(crate::dialogs::drawing_sheet::CLEAR_PRINTER),
            out,
        );
        ui.separator();
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
    // Whether CAD segments of one kind snap end to end into polylines
    // (manual p. 256).
    live(
        ui,
        "Connect CAD Segments",
        "",
        state.flags.contains(&ViewFlag::ConnectCad),
        Action::ToggleFlag(ViewFlag::ConnectCad),
        out,
    );
    ui.menu_button("Arc Creation Modes", |ui| {
        let current = crate::tools::cad::current_arc_mode();
        for m in crate::tools::cad::ArcMode::ALL {
            live(
                ui,
                m.name(),
                "",
                m == current,
                Action::Custom(m.command()),
                out,
            );
        }
    });
    // Moving a cabinet into its neighbours: Bump, Push or Pass Through (CB-4).
    live(
        ui,
        crate::tools::cabinet::bump_mode().toolbar_label(),
        "",
        false,
        Action::Custom(crate::tools::cabinet::BUMP_MODE_COMMAND),
        out,
    );
    ui.separator();
    ui.menu_button("Edit Area", |ui| {
        use crate::tools::select as sel;
        live(
            ui,
            "Edit Area",
            "",
            false,
            Action::Custom(sel::EDIT_AREA),
            out,
        );
        live(
            ui,
            "Edit Area Visible",
            "",
            false,
            Action::Custom(sel::EDIT_AREA_VISIBLE),
            out,
        );
    });
    live(
        ui,
        "Stretch CAD",
        "",
        false,
        Action::Custom(crate::tools::select::STRETCH_CAD),
        out,
    );
    ui.menu_button("Marquee Selection", |ui| {
        let current = crate::tools::select::marquee_mode();
        for m in crate::tools::select::MarqueeMode::ALL {
            live(
                ui,
                m.label(),
                "",
                m == current,
                Action::Custom(m.command()),
                out,
            );
        }
    });
    ui.separator();
    live(
        ui,
        "Find/Replace Text\u{2026}",
        "",
        false,
        Action::FindReplaceText,
        out,
    );
    live(
        ui,
        "Replace Fonts\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::find_replace::REPLACE_FONTS_PROMPT),
        out,
    );
    ui.separator();
    live(
        ui,
        "Default Settings\u{2026}",
        "",
        false,
        Action::DefaultSettings,
        out,
    );
    edit_row(
        ui,
        state,
        "Drawing Groups\u{2026}",
        "Drawing Groups",
        crate::tools::cad_ops::DG_DEFAULTS,
        out,
    );
    live(
        ui,
        "Watermark Defaults\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::watermark::DEFAULTS),
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
        "Material Defaults\u{2026}",
        "",
        false,
        Action::Custom(crate::tools::materials::DEFAULTS),
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
    live(
        ui,
        "Reset to Defaults",
        "",
        false,
        Action::Custom(crate::dialogs::defaults::RESET_TO_DEFAULTS),
        out,
    );
    live(
        ui,
        "Reverse Plan",
        "",
        false,
        Action::Custom(crate::shell::view_commands::REVERSE_PLAN),
        out,
    );
    inert(
        ui,
        &["-", "AutoFill>", "Start Dictation", "Emoji & Symbols"],
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
    edit_row(
        ui,
        state,
        "Multiple Copy\u{2026}",
        "Multiple Copy",
        crate::tools::cad_ops::MULTIPLE_COPY,
        out,
    );
    ui.menu_button("Drawing Group", |ui| {
        use crate::tools::cad_ops as ops;
        edit_row(
            ui,
            state,
            "Bring to Front",
            "Bring to Front",
            ops::DG_FRONT,
            out,
        );
        edit_row(ui, state, "Send to Back", "Send to Back", ops::DG_BACK, out);
        edit_row(
            ui,
            state,
            "Bring Forward",
            "Bring Forward",
            ops::DG_FORWARD,
            out,
        );
        edit_row(
            ui,
            state,
            "Send Backward",
            "Send Backward",
            ops::DG_BACKWARD,
            out,
        );
        edit_row(
            ui,
            state,
            "Set Drawing Group\u{2026}",
            "Set Drawing Group",
            ops::DG_SET,
            out,
        );
    });
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
                    if name == "Wall" {
                        ui.separator();
                        live(
                            ui,
                            "Define Wall Types\u{2026}",
                            "",
                            false,
                            Action::Custom(crate::dialogs::wall_types::OPEN),
                            out,
                        );
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
    // The Build Framing dialog: the next Framing window opens as Build Framing
    // (Floor, Ceiling, Roof, Wall, Posts, Trusses, Framing Defaults tabs).
    for (label, all) in [
        ("Build Framing\u{2026}", false),
        ("Build All Framing\u{2026}", true),
    ] {
        if ui.add(egui::Button::new(label)).clicked() {
            crate::dialogs::framing::request_build(all);
            out.push(Action::Custom(crate::dialogs::defaults::FRAMING));
            ui.close_menu();
        }
    }
    // CB-86: the framing of the deck rooms of the floor.
    for (label, id) in [
        (
            "Build Deck Framing",
            crate::editor::fireplace_view::cmd::BUILD_DECK,
        ),
        (
            "Delete Deck Framing",
            crate::editor::fireplace_view::cmd::CLEAR_DECK,
        ),
    ] {
        live(ui, label, "", false, Action::Custom(id), out);
    }
    // The edit commands of the Edit toolbar (manual pp. 914 to 943): they act on
    // the selection and say so in the status bar when it does not fit.
    {
        use crate::editor::framing_view::cmd;
        ui.separator();
        for (label, id) in [
            ("Build Framing for Selected Object(s)", cmd::BUILD_SELECTED),
            ("Build Framing for Parent Object(s)", cmd::BUILD_PARENT),
            ("Open Wall Detail", cmd::OPEN_WALL_DETAIL),
            ("Open Truss Detail", cmd::OPEN_TRUSS_DETAIL),
            ("Truss Detail Window\u{2026}", cmd::TRUSS_WINDOW),
            ("Find Trusses", cmd::FIND_TRUSSES),
            ("Move to Framing Ref", cmd::MOVE_TO_REF),
        ] {
            live(ui, label, "", false, Action::Custom(id), out);
        }
        ui.separator();
    }
    live(
        ui,
        "Framing Defaults\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::defaults::FRAMING),
        out,
    );
    // Brief 29: the catalogue dialogs and the member reporting.
    for (label, id) in [
        (
            "Automatic Framing Defaults\u{2026}",
            crate::dialogs::framing_defaults::AUTOMATIC,
        ),
        (
            "Manual Framing Defaults\u{2026}",
            crate::dialogs::framing_defaults::MANUAL,
        ),
        (
            "Framing Member Defaults\u{2026}",
            crate::dialogs::framing_defaults::MEMBERS,
        ),
        (
            "Framing Types\u{2026}",
            crate::dialogs::framing_defaults::TYPES,
        ),
        (
            "Structural Member Reporting\u{2026}",
            crate::dialogs::framing_defaults::REPORTING,
        ),
    ] {
        live(ui, label, "", false, Action::Custom(id), out);
    }
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
        live(
            ui,
            V::GlassHouse.label(),
            "",
            false,
            cmd(C::GlassHouse),
            out,
        );
        live(
            ui,
            V::FloorCamera.label(),
            "",
            false,
            cmd(C::Tool(V::FloorCamera)),
            out,
        );
        ui.separator();
        live(ui, "Ray Trace\u{2026}", "", false, cmd(C::RayTrace), out);
    });
    ui.menu_button("Create Orthographic View", |ui| {
        use crate::shell::view3d_panel::{IsoCorner, ParallelOverview as P};
        for p in [P::Full, P::Floor, P::Framing] {
            live(ui, p.label(), "", false, cmd(C::Parallel(p)), out);
        }
        ui.separator();
        for corner in IsoCorner::ALL {
            let p = P::Isometric(corner);
            live(ui, p.label(), "", false, cmd(C::Parallel(p)), out);
        }
    });
    ui.menu_button("Create Auto Elevations", |ui| {
        for side in crate::tools::camera::AutoSide::ALL {
            live(ui, side.label(), "", false, cmd(C::AutoSide(side)), out);
        }
        ui.separator();
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
    camera_step_menus(ui, out);
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
            "Walkthrough Path from CAD Polyline",
            "",
            false,
            cmd(C::WalkFromCad),
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
            "Object Eyedropper",
            "",
            mat::is_mode_active(mat::OBJECT_EYEDROPPER),
            Action::Custom(mat::OBJECT_EYEDROPPER),
            out,
        );
        live(
            ui,
            "Use Default Material",
            "",
            mat::is_mode_active(mat::USE_DEFAULT),
            Action::Custom(mat::USE_DEFAULT),
            out,
        );
        live(
            ui,
            "Adjust Material Definition",
            "",
            mat::is_mode_active(mat::ADJUST_DEFINITION),
            Action::Custom(mat::ADJUST_DEFINITION),
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
        live(ui, "Lighting\u{2026}", "", false, cmd(C::Lighting), out);
        ui.separator();
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
    live(ui, "Refresh", "", false, cmd(C::Refresh), out);
    live(ui, "Undo Zoom", "", false, cmd(C::UndoZoom), out);
    live(ui, "Save Camera", "", false, cmd(C::SaveCamera), out);
    ui.menu_button("View Quality", |ui| {
        for q in plan_core::camera_view::ViewQuality::ALL {
            live(ui, q.label(), "", false, cmd(C::Quality(q)), out);
        }
        ui.separator();
        live(
            ui,
            "Path-traced Final View",
            "",
            false,
            cmd(C::FinalViewToggle),
            out,
        );
    });
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
        live(
            ui,
            "360 Panorama\u{2026}",
            "",
            false,
            cmd(C::ExportPanorama),
            out,
        );
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

/// 3D > Move Camera with Mouse / with Keyboard, Move Camera, Orbit Camera,
/// Tilt Camera and View Direction (C-2, C-34, C-38, C-40, C-41). Isometric
/// Views need an orthographic camera at an angle the viewport does not have
/// yet (queue).
fn camera_step_menus(ui: &mut egui::Ui, out: &mut Vec<Action>) {
    use crate::shell::view3d_panel::nudge::{Direction, Nudge as N};
    use crate::shell::view3d_panel::View3dCommand as C;
    let step = |n| Action::View3d(C::Nudge(n));
    ui.menu_button("Move Camera with Mouse", |ui| {
        live(
            ui,
            "Mouse-Orbit Camera",
            "",
            false,
            Action::View3d(C::MouseOrbit),
            out,
        );
    });
    ui.menu_button("Move Camera with Keyboard", |ui| {
        for (name, key, n) in [
            ("Forward", "Up", N::Forward),
            ("Back", "Down", N::Back),
            ("Turn Left", "Left", N::TurnLeft),
            ("Turn Right", "Right", N::TurnRight),
            ("Raise", "PageUp", N::Raise),
            ("Lower", "PageDown", N::Lower),
        ] {
            live(ui, name, key, false, step(n), out);
        }
    });
    ui.menu_button("Move Camera", |ui| {
        for (name, n) in [
            ("Forward", N::Forward),
            ("Back", N::Back),
            ("Left", N::Left),
            ("Right", N::Right),
            ("Up", N::Raise),
            ("Down", N::Lower),
        ] {
            live(ui, name, "", false, step(n), out);
        }
    });
    ui.menu_button("Orbit Camera", |ui| {
        for (name, n) in [
            ("Left", N::OrbitLeft),
            ("Right", N::OrbitRight),
            ("Up", N::OrbitUp),
            ("Down", N::OrbitDown),
        ] {
            live(ui, name, "", false, step(n), out);
        }
    });
    ui.menu_button("Tilt Camera", |ui| {
        live(ui, "Tilt Up", "", false, step(N::TiltUp), out);
        live(ui, "Tilt Down", "", false, step(N::TiltDown), out);
    });
    ui.menu_button("View Direction", |ui| {
        for d in Direction::ALL {
            live(ui, d.label(), "", false, step(N::Look(d)), out);
        }
    });
}

/// Tools > Floor/Reference Display: Change Floor/Reference, Swap
/// Floor/Reference and Edit Reference Document Offset (manual pp. 89-91).
fn reference_menu(ui: &mut egui::Ui, out: &mut Vec<Action>) {
    use crate::dialogs::reference_display as rd;
    for (label, cmd) in [
        ("Change Floor/Reference\u{2026}", rd::CHANGE),
        ("Swap Floor/Reference", rd::SWAP),
        ("Edit Reference Document Offset", rd::EDIT_OFFSET),
    ] {
        live(ui, label, "", false, Action::Custom(cmd), out);
    }
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
    // Construction lines: the rule sets that number them and the defaults.
    for (label, cmd) in [
        (
            "Construction Line Order Management\u{2026}",
            crate::dialogs::construction_line::ORDER,
        ),
        (
            "Construction Line Defaults\u{2026}",
            crate::dialogs::construction_line::DEFAULTS,
        ),
    ] {
        live(ui, label, "", false, Action::Custom(cmd), out);
    }
    live(
        ui,
        "Line Style Management\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::line_style::MANAGEMENT),
        out,
    );
    live(
        ui,
        "Line Style (Library) for Selection",
        "",
        false,
        Action::Custom(crate::dialogs::line_style::ASSIGN),
        out,
    );
    cad_mode(ui, "Revision Cloud", CadMode::RevisionCloud, out);
    cad_mode(ui, "Spline", CadMode::Spline, out);
    // Typing lines and arcs by bearing and distance, and the Number Style.
    ui.menu_button("Survey Entry", |ui| {
        for (label, cmd) in crate::tools::cad::survey::MENU {
            live(ui, label, "", false, Action::Custom(cmd), out);
        }
    });
    ui.separator();
    toolbar::flyout_menu(ui, &toolbar::dimensions(), state, out);
    toolbar::flyout_menu(ui, &toolbar::auto_dimensions(), state, out);
    ui.separator();
    toolbar::flyout_menu(ui, &toolbar::text_tools(), state, out);
    ui.menu_button("Patterns", |ui| {
        cad_mode(ui, "Hatch Closed Shape", CadMode::Hatch, out);
        ui.separator();
        // Custom patterns (CAD-79..CAD-81) and fill styles (CAD-73).
        for (label, cmd) in [
            (
                "Create New Pattern\u{2026}",
                crate::dialogs::pattern_editor::CREATE,
            ),
            ("Edit Pattern\u{2026}", crate::dialogs::pattern_editor::EDIT),
            (
                "Add Pattern to Library",
                crate::dialogs::pattern_editor::ADD_TO_LIBRARY,
            ),
            (
                "Next Pattern Tile Group",
                crate::dialogs::pattern_editor::NEXT_GROUP,
            ),
            (
                "Previous Pattern Tile Group",
                crate::dialogs::pattern_editor::PREVIOUS_GROUP,
            ),
            (
                "Add Pattern Tile Group",
                crate::dialogs::pattern_editor::ADD_GROUP,
            ),
            (
                "Delete Pattern Tile Group",
                crate::dialogs::pattern_editor::DELETE_GROUP,
            ),
            (
                "Infinite Pattern Line",
                crate::dialogs::pattern_editor::INFINITE_LINE,
            ),
        ] {
            live(ui, label, "", false, Action::Custom(cmd), out);
        }
        ui.separator();
        live(
            ui,
            "Fill Style\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::fill_style::APPLY),
            out,
        );
        live(
            ui,
            "New Fill Style\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::fill_style::NEW_NAMED),
            out,
        );
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
            CadMode::ChangeLineArc,
            CadMode::DeleteBreak,
            CadMode::DisconnectEdges,
            CadMode::HideShowEdge,
            CadMode::MakeArcTangent,
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
            CadMode::JoinTwoLines,
            CadMode::ClosePolyline,
            CadMode::SimplifyPolyline,
            CadMode::FilletAllCorners,
        ] {
            cad_mode(ui, m.name(), m, out);
        }
        // Closed polylines become polygon soffits (CB-17).
        live(
            ui,
            "Convert Polyline to Soffit",
            "",
            false,
            Action::Custom(crate::editor::placed::SOFFIT_FROM_POLYLINE),
            out,
        );
        ui.separator();
        use crate::tools::cad_ops as ops;
        edit_row(
            ui,
            state,
            "Polyline Union",
            "Polyline Union",
            ops::UNION,
            out,
        );
        edit_row(
            ui,
            state,
            "Polyline Subtract",
            "Polyline Subtract",
            ops::SUBTRACT,
            out,
        );
        edit_row(
            ui,
            state,
            "Polyline Intersect",
            "Polyline Intersect",
            ops::INTERSECT,
            out,
        );
        ui.separator();
        edit_row(
            ui,
            state,
            "Trim to Boundary",
            "Trim to Boundary",
            ops::TRIM_BOUNDARY,
            out,
        );
        edit_row(
            ui,
            state,
            "Extend to Boundary",
            "Extend to Boundary",
            ops::EXTEND_BOUNDARY,
            out,
        );
        edit_row(
            ui,
            state,
            "Insert Point",
            "Insert Point",
            ops::INSERT_POINT,
            out,
        );
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
    live(
        ui,
        "Auto Detail",
        "",
        false,
        Action::Custom(crate::tools::details::AUTO_DETAIL),
        out,
    );
    live(
        ui,
        "CAD Detail From View",
        "",
        false,
        Action::Custom(crate::tools::details::DETAIL_FROM_VIEW),
        out,
    );
    live(
        ui,
        "CAD Detail Management\u{2026}",
        "",
        false,
        Action::Custom(crate::tools::details::MANAGEMENT),
        out,
    );
    live(
        ui,
        "Detail Components\u{2026}",
        "",
        false,
        Action::Custom(crate::tools::details::COMPONENTS),
        out,
    );
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
        live(
            ui,
            "Layer Set Defaults\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::layer_sets::DEFAULTS),
            out,
        );
    });
    ui.menu_button("Floor/Reference Display", |ui| reference_menu(ui, out));
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
        live(
            ui,
            "Add Starter Plan Views",
            "",
            false,
            Action::Custom(crate::dialogs::plan_views::STARTER),
            out,
        );
        ui.separator();
        live(
            ui,
            "New Saved Plan View\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::plan_views::NEW_SAVED),
            out,
        );
        live(
            ui,
            "Save Active View As\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::plan_views::SAVE_AS),
            out,
        );
    });
    live(
        ui,
        "Active Defaults\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::default_sets::ACTIVE_DEFAULTS),
        out,
    );
    live(
        ui,
        "Default Sets\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::default_sets::DEFAULT_SETS),
        out,
    );
    live(
        ui,
        "Rotate Plan View\u{2026}",
        "",
        false,
        Action::Custom(crate::shell::view_commands::ROTATE_DIALOG),
        out,
    );
    live(
        ui,
        "Reverse Plan",
        "",
        false,
        Action::Custom(crate::shell::view_commands::REVERSE_PLAN),
        out,
    );
    ui.separator();
    ui.menu_button("Checks", |ui| {
        live(ui, "Plan Check", "", false, Action::PlanCheck, out);
        live(
            ui,
            "Reset Notification Icons",
            "",
            false,
            Action::Custom(crate::editor::wall_edit::RESET_ICONS),
            out,
        );
        live(
            ui,
            "Plan Check Settings\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::plan_check::SETTINGS),
            out,
        );
        live(
            ui,
            "Check While Drawing",
            "",
            crate::dialogs::preferences::pages::current()
                .architectural
                .check_while_drawing,
            Action::Custom(crate::dialogs::plan_check::CHECK_LIVE),
            out,
        );
        live(
            ui,
            "Apply Code Minimums to Defaults",
            "",
            false,
            Action::Custom(crate::dialogs::plan_check::APPLY_DEFAULTS),
            out,
        );
        live(
            ui,
            "Door/Window Check",
            "",
            false,
            Action::DoorWindowCheck,
            out,
        );
        live(
            ui,
            "Kitchen and Bath Report",
            "",
            false,
            Action::Custom(crate::dialogs::plan_check::NKBA_REPORT),
            out,
        );
        live(
            ui,
            "Kitchen and Bath Report to Excel\u{2026}",
            "",
            false,
            Action::Custom(crate::dialogs::plan_check::NKBA_XLSX),
            out,
        );
        live(ui, "Plan Footprint", "", false, Action::PlanFootprint, out);
    });
    ui.menu_button("Calculators", |ui| {
        for (label, id) in [
            ("Header/Beam\u{2026}", crate::dialogs::calculators::HEADER),
            ("Joist Span\u{2026}", crate::dialogs::calculators::JOIST),
            ("Rafter Span\u{2026}", crate::dialogs::calculators::RAFTER),
            ("Stair\u{2026}", crate::dialogs::calculators::STAIR),
            ("Deck Beam/Joist\u{2026}", crate::dialogs::calculators::DECK),
        ] {
            live(ui, label, "", false, Action::Custom(id), out);
        }
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
        // Marks set in the order the doors / windows were drawn (DW-61).
        live(
            ui,
            "Renumber Door Schedule",
            "",
            false,
            Action::Custom(crate::editor::opening_edit::RENUMBER_DOORS),
            out,
        );
        live(
            ui,
            "Renumber Window Schedule",
            "",
            false,
            Action::Custom(crate::editor::opening_edit::RENUMBER_WINDOWS),
            out,
        );
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
        // The settings new schedules of each type start with.
        ui.menu_button("Schedule Defaults", |ui| {
            for k in crate::tools::schedule::FLYOUT_KINDS {
                live(
                    ui,
                    crate::tools::schedule::entry_name(k),
                    "",
                    false,
                    Action::Custom(crate::editor::schedule_view::defaults_command(k)),
                    out,
                );
            }
        });
        // Create, rename and delete the plan's custom schedule categories.
        live(
            ui,
            "Manage Custom Schedule Categories\u{2026}",
            "",
            false,
            Action::Custom(crate::editor::schedule_view::cmd::MANAGE_CATEGORIES),
            out,
        );
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
    ui.menu_button("Materials List", |ui| {
        use crate::dialogs::materials_list::cmd;
        live(
            ui,
            "Calculate Materials for All Floors",
            "",
            false,
            Action::Custom(cmd::ALL),
            out,
        );
        live(
            ui,
            "Calculate Materials From Selection",
            "",
            false,
            Action::Custom(cmd::SELECTION),
            out,
        );
        live(
            ui,
            "Calculate Materials in Room",
            "",
            false,
            Action::Custom(cmd::ROOM),
            out,
        );
        live(
            ui,
            "Materials List Polyline",
            "",
            state.tool == ToolId::MaterialsPolyline,
            Action::SetTool(ToolId::MaterialsPolyline),
            out,
        );
        live(
            ui,
            "Materials List Polyline Defaults\u{2026}",
            "",
            false,
            Action::Custom(cmd::POLYLINE_DEFAULTS),
            out,
        );
        ui.separator();
        live(
            ui,
            "Master List",
            "",
            false,
            Action::Custom(cmd::MASTER),
            out,
        );
        live(
            ui,
            "Materials List Management\u{2026}",
            "",
            false,
            Action::Custom(cmd::MANAGE),
            out,
        );
        live(
            ui,
            "Generate a Report",
            "",
            false,
            Action::Custom(cmd::REPORT),
            out,
        );
        ui.separator();
        live(
            ui,
            "Open Materials List\u{2026}",
            "",
            false,
            Action::MaterialsList,
            out,
        );
        live(
            ui,
            "Edit Active View\u{2026}",
            "",
            false,
            Action::Custom(cmd::EDIT_VIEW),
            out,
        );
        live(
            ui,
            "Save Active View",
            "",
            false,
            Action::Custom(cmd::SAVE),
            out,
        );
        live(
            ui,
            "Save Active View As\u{2026}",
            "",
            false,
            Action::Custom(cmd::SAVE_AS),
            out,
        );
        live(
            ui,
            "Update From Master List",
            "",
            false,
            Action::Custom(cmd::UPDATE_FROM),
            out,
        );
        live(
            ui,
            "Update To Master List",
            "",
            false,
            Action::Custom(cmd::UPDATE_TO),
            out,
        );
        live(
            ui,
            "Export Materials List\u{2026}",
            "",
            false,
            Action::Custom(cmd::EXPORT),
            out,
        );
        live(
            ui,
            "Print Materials List\u{2026}",
            "",
            false,
            Action::Custom(cmd::PRINT),
            out,
        );
    });
    ui.menu_button("Layer Painter", |ui| {
        use crate::tools::painters::PainterMode as P;
        for m in [P::LayerPaint, P::LayerEyedropper, P::LayerHider] {
            let id = ToolId::PainterVariant(m);
            live(ui, m.name(), "", state.tool == id, Action::SetTool(id), out);
        }
    });
    ui.menu_button("Object Painter", |ui| {
        use crate::tools::painters::PainterMode as P;
        for m in [P::ObjectPaint, P::ObjectEyedropper] {
            let id = ToolId::PainterVariant(m);
            live(ui, m.name(), "", state.tool == id, Action::SetTool(id), out);
        }
        ui.separator();
        live(
            ui,
            "Object Painter Modes\u{2026}",
            "",
            false,
            Action::Custom(crate::tools::painters::MODES),
            out,
        );
    });
    live(
        ui,
        "Spell Check\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::spell_check::OPEN),
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
    // Beyond Chief: custom properties and the Excel exchange.
    live(
        ui,
        "Property Manager\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::property_manager::OPEN),
        out,
    );
    live(
        ui,
        "Export Property Data (XLSX)\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::property_manager::EXPORT_ALL),
        out,
    );
    live(
        ui,
        "Import Property Data (XLSX)\u{2026}",
        "",
        false,
        Action::Custom(crate::dialogs::property_manager::IMPORT),
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
        "Plan Agent\u{2026}",
        "",
        dock(Dock::Agent),
        Action::ToggleDock(Dock::Agent),
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
        "Poch\u{e9}",
        "",
        crate::dialogs::fill_style::poche_shown(),
        Action::Custom(crate::dialogs::fill_style::POCHE),
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
    live(
        ui,
        "Watermark",
        "",
        crate::dialogs::watermark::menu_checked(),
        Action::Custom(crate::dialogs::watermark::TOGGLE),
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

/// A Window or File row that runs the view command `id`.
fn vrow(ui: &mut egui::Ui, label: &str, hk: &str, id: &'static str, out: &mut Vec<Action>) {
    live(ui, label, hk, false, Action::Custom(id), out);
}

fn window_menu(ui: &mut egui::Ui, state: &BarState, out: &mut Vec<Action>) {
    // Daniel's "-" is Zoom In; the labels follow the live hotkey map.
    let key = |name: &str| state.hotkey(name, "");
    use crate::shell::view_commands as vc;
    vrow(ui, "Zoom", &key("Zoom"), vc::ZOOM_WINDOW, out);
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
    vrow(ui, "Zoom Previous", "", vc::ZOOM_PREVIOUS, out);
    live(
        ui,
        "Fill Window Selected Objects",
        "",
        false,
        Action::Custom(crate::dialogs::app_info::FILL_SELECTED),
        out,
    );
    vrow(ui, "Fill Window Building Only", "", vc::FILL_BUILDING, out);
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
    ui.menu_button("Rotate Plan View", |ui| {
        vrow(ui, "90 Degrees Left", "", vc::ROTATE_LEFT, out);
        vrow(ui, "90 Degrees Right", "", vc::ROTATE_RIGHT, out);
        vrow(ui, "Back to North Up", "", vc::ROTATE_RESET, out);
        vrow(ui, "Rotate to an Angle\u{2026}", "", vc::ROTATE_DIALOG, out);
    });
    ui.separator();
    vrow(ui, "Swap Views", "F7", vc::SWAP_VIEWS, out);
    vrow(ui, "Tile Horizontally", "", vc::TILE_HORIZONTALLY, out);
    vrow(ui, "Tile Vertically", "", vc::TILE_VERTICALLY, out);
    vrow(ui, "Tab Windows", "", vc::TAB_WINDOWS, out);
    vrow(ui, "Select Next Tab", "\u{2303}\u{21E5}", vc::NEXT_TAB, out);
    vrow(
        ui,
        "Select Previous Tab",
        "\u{21E7}\u{2303}\u{21E5}",
        vc::PREVIOUS_TAB,
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
    row(ui, "Center Object", C::CenterObject, out);
    row(
        ui,
        "Point to Point Move",
        C::Tool(crate::shell::layout_window::LayoutTool::PointToPoint),
        out,
    );
    row(ui, "Open Source View", C::OpenSourceView, out);
    row(ui, "Copy Layout Box to Page\u{2026}", C::CopyBoxToPage, out);
    row(ui, "Duplicate Layout Box", C::DuplicateBox, out);
    ui.menu_button("Align Layout Boxes", |ui| {
        for edge in plan_layout::AlignEdge::ALL {
            row(ui, edge.label(), C::Align(edge), out);
        }
        ui.separator();
        row(
            ui,
            "Spread Horizontally",
            C::Distribute(plan_layout::Spread::Horizontal),
            out,
        );
        row(
            ui,
            "Spread Vertically",
            C::Distribute(plan_layout::Spread::Vertical),
            out,
        );
    });
    ui.menu_button("Update Layout Views", |ui| {
        row(ui, "Update All Views", C::UpdateViews, out);
        row(ui, "Update All Live Views", C::UpdateLiveViews, out);
        row(
            ui,
            "Update All Plot Line Views",
            C::UpdatePlotLineViews,
            out,
        );
        row(ui, "Update Selected View", C::UpdateView, out);
    });
    row(ui, "Send All Views to Layout\u{2026}", C::SendAllViews, out);
    ui.menu_button("Edit Layout View", |ui| {
        row(ui, "Rescale Layout View\u{2026}", C::RescaleView, out);
        row(
            ui,
            "Pan/Scale Layout Box",
            C::Tool(crate::shell::layout_window::LayoutTool::PanScale),
            out,
        );
        row(ui, "Recenter Layout Box Contents", C::RecenterBox, out);
        row(
            ui,
            "Scale Layout Box Contents to Fit",
            C::ScaleBoxToFit,
            out,
        );
        row(ui, "Layout Box Layers\u{2026}", C::LayoutBoxLayers, out);
        row(ui, "Unlink Saved Plan View", C::UnlinkSavedView, out);
        row(
            ui,
            "Edit Layout Lines",
            C::Tool(crate::shell::layout_window::LayoutTool::EditLines),
            out,
        );
    });
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
    row(ui, "Page Information\u{2026}", C::PageInformation, out);
    row(ui, "Layout Revision Table", C::RevisionTable, out);
    row(ui, "Add Layout Revision\u{2026}", C::AddLayoutRevision, out);
    row(
        ui,
        "Copy Drawings to Page\u{2026}",
        C::CopyDrawingsToPage,
        out,
    );
    row(
        ui,
        "General Layout Defaults\u{2026}",
        C::LayoutDefaults,
        out,
    );
    ui.separator();
    row(ui, "Page Setup\u{2026}", C::PageSetup, out);
    row(
        ui,
        "Customize Sheet Sizes\u{2026}",
        C::CustomizeSheetSizes,
        out,
    );
    row(ui, "Project Information\u{2026}", C::ProjectInfo, out);
    row(ui, "Fit Page in Window", C::FitPage, out);
    row(ui, "Layer Display Options\u{2026}", C::LayerDisplay, out);
    row(ui, "Add Sheet Index", C::AddSheetIndex, out);
    ui.separator();
    row(ui, "Save As Template\u{2026}", C::SaveAsTemplate, out);
    row(ui, "Apply Template\u{2026}", C::ApplyTemplate, out);
    row(ui, "New Layout File\u{2026}", C::NewLayoutFile, out);
    ui.separator();
    row(ui, "Export Table as CSV\u{2026}", C::ExportTableCsv, out);
    row(
        ui,
        "Export Table to Excel\u{2026}",
        C::ExportTableExcel,
        out,
    );
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
            crate::tools::images::IMPORT_PICTURE,
            crate::tools::details::MANAGEMENT,
            crate::tools::details::COMPONENTS,
            crate::tools::details::DETAIL_FROM_VIEW,
            crate::tools::details::AUTO_DETAIL,
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
            crate::dialogs::plan_check::SETTINGS,
            crate::tools::molding::NEXT_EDGE,
            crate::tools::molding::INCLUDE_INSIDE,
            crate::tools::molding::PLACE_PROFILE,
        ] {
            assert!(placed::run_command(&mut cx, id), "{id} is not handled");
        }
        mat::set_painter_mode(mat::PainterMode::Off);
    }
}
