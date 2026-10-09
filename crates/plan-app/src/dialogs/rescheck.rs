//! File > Export > Thermal Envelope Data and Export to REScheck (L-83;
//! reference manual pp. 1304 to 1306).
//!
//! * Thermal Envelope Data writes a comma-delimited `.csv` of the envelope by
//!   floor level (direction and area of every wall, door and window, the
//!   floor and ceiling platforms with their assemblies) and opens it in the
//!   program that handles `.csv` files.
//! * Export to REScheck opens a small dialog with Group Similar Walls and
//!   Group Similar Doors/Windows, then a Save File dialog, and writes the
//!   `.rxl`. The project data comes from Project Information; the envelope
//!   from `plan_docs::rescheck`.
//!
//! The menu bar calls [`show_windows`] each frame (it draws the options
//! dialog and posts [`EXPORT_RESCHECK_RUN`] when Export is pressed); the
//! commands run through `tools::text::run_command`.

use crate::editor::EditorContext;
use crate::toolbar::Action;
use eframe::egui;
use plan_docs::rescheck::{self, Inputs, Options};
use std::cell::RefCell;
use std::path::Path;

/// File > Export > Thermal Envelope Data.
pub const EXPORT_THERMAL: &str = "text.export_thermal";
/// File > Export > Export to REScheck: opens the options dialog.
pub const EXPORT_RESCHECK: &str = "text.export_rescheck";
/// Posted by the dialog's Export button.
pub const EXPORT_RESCHECK_RUN: &str = "text.export_rescheck_run";

pub fn is_command(id: &str) -> bool {
    matches!(id, EXPORT_THERMAL | EXPORT_RESCHECK | EXPORT_RESCHECK_RUN)
}

#[derive(Default)]
struct Dialog {
    open: bool,
    group_walls: bool,
    group_openings: bool,
}

thread_local! {
    static DIALOG: RefCell<Dialog> = RefCell::new(Dialog {
        open: false,
        group_walls: true,
        group_openings: true,
    });
}

/// What the export reads besides the plan: the room types (which rooms are
/// conditioned) and the wall types of the program's defaults.
pub fn inputs(cx: &EditorContext) -> Inputs<'_> {
    Inputs {
        room_types: &cx.defaults.rooms.room_types,
        wall_types: &cx.defaults.wall_types,
    }
}

/// The file text of Thermal Envelope Data.
pub fn thermal_csv(cx: &EditorContext) -> String {
    let env = rescheck::envelope(&cx.project, &inputs(cx), &Options::default());
    rescheck::thermal_csv(&env)
}

/// The file text of an REScheck export with the dialog's choices.
pub fn rescheck_rxl(cx: &EditorContext, opts: &Options) -> String {
    rescheck::to_rxl(&cx.project, &inputs(cx), opts)
}

/// Writes Thermal Envelope Data to `path`.
pub fn export_thermal_to(cx: &mut EditorContext, path: &Path) -> Result<(), String> {
    let text = thermal_csv(cx);
    std::fs::write(path, text).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    cx.status = format!("Saved {}", path.display());
    Ok(())
}

/// Writes the REScheck file to `path`.
pub fn export_rescheck_to(
    cx: &mut EditorContext,
    path: &Path,
    opts: &Options,
) -> Result<(), String> {
    let xml = rescheck_rxl(cx, opts);
    rescheck::well_formed(&xml)?;
    std::fs::write(path, xml).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    cx.status = format!("Saved {}", path.display());
    Ok(())
}

/// The options the dialog holds now.
pub fn dialog_options() -> Options {
    DIALOG.with(|d| {
        let d = d.borrow();
        Options {
            group_walls: d.group_walls,
            group_openings: d.group_openings,
            ..Options::default()
        }
    })
}

/// Sets the dialog's checkboxes (tests and shortcuts).
pub fn set_dialog_options(group_walls: bool, group_openings: bool) {
    DIALOG.with(|d| {
        let mut d = d.borrow_mut();
        d.group_walls = group_walls;
        d.group_openings = group_openings;
    });
}

pub fn dialog_open() -> bool {
    DIALOG.with(|d| d.borrow().open)
}

pub fn close_dialog() {
    DIALOG.with(|d| d.borrow_mut().open = false);
}

/// Runs a command of this module. False for a command that is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        EXPORT_THERMAL => {
            let name = format!("{}_thermal_envelope.csv", file_stem(cx));
            let Some(path) = rfd::FileDialog::new()
                .set_file_name(&name)
                .add_filter("csv", &["csv"])
                .save_file()
            else {
                cx.status = "Export cancelled".into();
                return true;
            };
            match export_thermal_to(cx, &path) {
                // It opens in the program that handles .csv files.
                Ok(()) => {
                    let _ = crate::tools::text::follow_hyperlink(&path.display().to_string());
                }
                Err(e) => cx.status = e,
            }
            true
        }
        EXPORT_RESCHECK => {
            DIALOG.with(|d| d.borrow_mut().open = true);
            true
        }
        EXPORT_RESCHECK_RUN => {
            let opts = dialog_options();
            close_dialog();
            let name = format!("{}.rxl", file_stem(cx));
            let Some(path) = rfd::FileDialog::new()
                .set_file_name(&name)
                .add_filter("REScheck", &["rxl"])
                .save_file()
            else {
                cx.status = "Export cancelled".into();
                return true;
            };
            if let Err(e) = export_rescheck_to(cx, &path, &opts) {
                cx.status = e;
            }
            true
        }
        _ => false,
    }
}

fn file_stem(cx: &EditorContext) -> String {
    let n: String = cx
        .project
        .name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if n.is_empty() {
        "plan".into()
    } else {
        n
    }
}

/// Draws the Export to REScheck dialog when it is up; Export posts
/// [`EXPORT_RESCHECK_RUN`] into `out`.
pub fn show_windows(ctx: &egui::Context, out: &mut Vec<Action>) {
    if !dialog_open() {
        return;
    }
    let mut state = DIALOG.with(|d| std::mem::take(&mut *d.borrow_mut()));
    let mut open = state.open;
    let mut export = false;
    egui::Window::new("Export to REScheck")
        .id(egui::Id::new("export_to_rescheck"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("REScheck reads the thermal envelope of the plan.");
            ui.checkbox(
                &mut state.group_walls,
                "Group Similar Walls (walls of one direction and assembly on every floor are added together)",
            );
            ui.checkbox(
                &mut state.group_openings,
                "Group Similar Doors/Windows (those of one direction and properties are added together)",
            );
            ui.weak("Location and permit data are not exported; skylights are not exported.");
            ui.horizontal(|ui| {
                if ui.button("Export").clicked() {
                    export = true;
                }
                if ui.button("Cancel").clicked() {
                    state.open = false;
                }
            });
        });
    state.open = state.open && open;
    if export {
        out.push(Action::Custom(EXPORT_RESCHECK_RUN));
    }
    DIALOG.with(|d| *d.borrow_mut() = state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::model::{OpeningKind, WallKind};

    fn cx_with_box() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let pts = [(0.0, 0.0), (240.0, 0.0), (240.0, 180.0), (0.0, 180.0)];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            cx.project.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        let w = cx.project.floors[0].walls[0].id;
        cx.project
            .add_opening(0, w, 120.0, OpeningKind::Window)
            .unwrap();
        cx
    }

    #[test]
    fn the_exports_write_a_csv_and_a_well_formed_rxl() {
        let mut cx = cx_with_box();
        let dir = std::env::temp_dir().join(format!("rescheck_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("t.csv");
        export_thermal_to(&mut cx, &csv).unwrap();
        let text = std::fs::read_to_string(&csv).unwrap();
        assert!(text.starts_with("Floor Level,Component,"));
        assert!(text.contains("Window"));
        let rxl = dir.join("t.rxl");
        export_rescheck_to(&mut cx, &rxl, &Options::default()).unwrap();
        let xml = std::fs::read_to_string(&rxl).unwrap();
        rescheck::well_formed(&xml).unwrap();
        assert!(xml.contains("<FrontFaces>South</FrontFaces>"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_dialog_holds_the_grouping_choices_and_draws() {
        let mut cx = cx_with_box();
        assert!(run_command(&mut cx, EXPORT_RESCHECK));
        assert!(dialog_open());
        assert_eq!(dialog_options(), Options::default());
        set_dialog_options(false, true);
        assert!(!dialog_options().group_walls);
        let ctx = egui::Context::default();
        let mut out = Vec::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| show_windows(ctx, &mut out));
        assert!(out.is_empty());
        close_dialog();
        set_dialog_options(true, true);
        assert!(is_command(EXPORT_THERMAL) && !is_command("edit.copy"));
        assert!(!run_command(&mut cx, "text.nothing"));
    }
}
