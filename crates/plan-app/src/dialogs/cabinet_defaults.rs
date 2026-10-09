//! Default Settings > Cabinets > General Cabinet Defaults (CB-476, reference
//! manual pp. 644 and 645; reached from Default Settings only).
//!
//! The dialog edits [`plan_core::defaults::GeneralCabinetDefaults`]:
//!
//! * Automatic Behaviors: Minimum Cabinet Width (not under 1/16 in), Minimum
//!   Shelf Spacing, Auto Door Threshold, Create Automatic Fillers, Create
//!   Automatic Fillers for Angled Connections, Create Automatic Blind Corner
//!   Cabinets. Create Automatic Fillers is dynamic: OK rebuilds the fillers of
//!   the plan's cabinets as one undo step.
//! * Cabinet Resizing: Use Grid Snaps or Use Resize Increment, and the
//!   increment (not under 1/16 in).
//! * Plan Display Options: Show Partial Module Lines, Show Closed
//!   Doors/Drawers and Panels, Show Pilasters, Display Molding Edges.

use super::{fmt_short, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use crate::editor::EditorContext;
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Ui};
use plan_core::defaults::GeneralCabinetDefaults;
use std::cell::{Cell, RefCell};

const TABS: &[Tab] = &[on("General")];

/// The General Cabinet Defaults dialog.
pub struct GeneralCabinetDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: GeneralCabinetDefaults,
    fields: Fields,
}

impl GeneralCabinetDialog {
    pub fn new(defaults: &GeneralCabinetDefaults) -> Self {
        Self {
            frame: SpecDialog::new("General Cabinet Defaults", "general_cabinet_defaults"),
            form: Form {
                draft: defaults.clone(),
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &GeneralCabinetDefaults {
        &self.form.draft
    }

    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut GeneralCabinetDefaults {
        &mut self.form.draft
    }

    /// The reason OK is refused, if any.
    #[cfg(test)]
    pub fn problem(&self) -> Option<String> {
        self.form.error()
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Enter valid lengths".into());
        }
        let d = &self.draft;
        let smallest = GeneralCabinetDefaults::SMALLEST;
        if d.min_cabinet_width < smallest - 1e-9 {
            return Some("The Minimum Cabinet Width cannot be under 1/16\"".into());
        }
        if d.resize_increment < smallest - 1e-9 {
            return Some("The Resize Increment cannot be under 1/16\"".into());
        }
        if d.min_shelf_spacing < 0.0 || d.auto_door_threshold < 0.0 {
            return Some("Spacings cannot be negative".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, _tab: usize) {
        let (f, d) = (&mut self.fields, &mut self.draft);
        section(ui, "Automatic Behaviors");
        f.length_row(
            ui,
            "Minimum Cabinet Width",
            "gcd_min_w",
            &mut d.min_cabinet_width,
        );
        f.length_row(
            ui,
            "Minimum Shelf Spacing",
            "gcd_shelf",
            &mut d.min_shelf_spacing,
        );
        f.length_row(
            ui,
            "Auto Door Threshold",
            "gcd_auto_door",
            &mut d.auto_door_threshold,
        );
        ui.checkbox(&mut d.create_automatic_fillers, "Create Automatic Fillers");
        ui.add_enabled(
            d.create_automatic_fillers,
            egui::Checkbox::new(
                &mut d.create_automatic_fillers_angled,
                "Create Automatic Fillers for Angled Connections",
            ),
        );
        ui.checkbox(
            &mut d.create_automatic_blind_corners,
            "Create Automatic Blind Corner Cabinets",
        );
        section(ui, "Cabinet Resizing");
        row(ui, "Resize By", |ui| {
            ui.radio_value(&mut d.resize_by_grid, true, "Use Grid Snaps");
            ui.radio_value(&mut d.resize_by_grid, false, "Use Resize Increment");
        });
        ui.add_enabled_ui(!d.resize_by_grid, |ui| {
            f.length_row(ui, "Resize Increment", "gcd_incr", &mut d.resize_increment);
        });
        section(ui, "Plan Display Options");
        ui.checkbox(
            &mut d.show_partial_module_lines,
            "Show Partial Module Lines",
        );
        ui.checkbox(
            &mut d.show_closed_doors_drawers,
            "Show Closed Doors/Drawers and Panels",
        );
        ui.checkbox(&mut d.show_pilasters, "Show Pilasters");
        ui.checkbox(
            &mut d.display_molding_edges,
            "Display Molding Edges in Plan Views",
        );
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let d = &self.draft;
        let lines = [
            format!("Minimum width {}", fmt_short(d.min_cabinet_width)),
            if d.resize_by_grid {
                "Resize with the Snap Grid".to_string()
            } else {
                format!("Resize by {}", fmt_short(d.resize_increment))
            },
            if d.create_automatic_fillers {
                "Automatic fillers on".to_string()
            } else {
                "Automatic fillers off".to_string()
            },
            if d.show_partial_module_lines {
                "Partial module lines".to_string()
            } else {
                "Full module lines".to_string()
            },
        ];
        for (i, text) in lines.iter().enumerate() {
            pv_text(
                painter,
                Pos2::new(rect.center().x, rect.min.y + 14.0 + 16.0 * i as f32),
                Align2::CENTER_CENTER,
                text,
                11.0,
            );
        }
    }
}

thread_local! {
    static OPEN: Cell<bool> = const { Cell::new(false) };
    static PAGE: RefCell<Option<GeneralCabinetDialog>> = const { RefCell::new(None) };
}

/// Asks for the dialog; [`show`] opens it on its next frame.
pub fn request_open() {
    OPEN.with(|c| c.set(true));
}

/// Is the dialog asked for or showing?
#[cfg(test)]
pub fn is_open() -> bool {
    OPEN.with(Cell::get) || PAGE.with(|p| p.borrow().is_some())
}

/// Draws the dialog (when open) and applies OK: the values are stored in the
/// plan's defaults, and the automatic fillers follow (Create Automatic
/// Fillers is dynamic) as one undo step, "General Cabinet Defaults".
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if OPEN.with(|c| c.replace(false)) {
        let dlg = GeneralCabinetDialog::new(&cx.defaults.cabinets.general);
        PAGE.with(|p| *p.borrow_mut() = Some(dlg));
    }
    let Some(mut dlg) = PAGE.with(|p| p.borrow_mut().take()) else {
        return;
    };
    match dlg.show(ctx) {
        Outcome::Open => PAGE.with(|p| *p.borrow_mut() = Some(dlg)),
        Outcome::Cancel => {}
        Outcome::Ok => {
            apply(cx, dlg.draft());
        }
    }
}

/// Stores `general` as the plan's General Cabinet Defaults and rebuilds the
/// automatic fillers to match. Returns whether a cabinet changed.
pub fn apply(cx: &mut EditorContext, general: &GeneralCabinetDefaults) -> bool {
    cx.defaults.cabinets.general = general.clamped();
    cx.begin_change("General Cabinet Defaults");
    let changed = crate::tools::cabinet::sync_auto_fillers(cx) > 0;
    if changed {
        crate::editor::placed::rejoin_if_enabled(cx);
        cx.mark_dirty();
    } else {
        cx.cancel_change();
        cx.mark_dirty();
    }
    cx.status = "Saved the General Cabinet Defaults".into();
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_refuses_values_under_a_sixteenth() {
        let mut dlg = GeneralCabinetDialog::new(&GeneralCabinetDefaults::default());
        assert_eq!(dlg.problem(), None);
        dlg.draft_mut().min_cabinet_width = 0.03;
        assert!(dlg.problem().is_some());
        dlg.draft_mut().min_cabinet_width = 1.0 / 16.0;
        assert_eq!(dlg.problem(), None);
        dlg.draft_mut().resize_increment = 0.0;
        assert!(dlg.problem().is_some());
    }

    #[test]
    fn defaults_match_the_behaviour_before_the_dialog_existed() {
        let g = GeneralCabinetDefaults::default();
        assert_eq!((g.min_cabinet_width, g.resize_increment), (3.0, 3.0));
        assert!(g.create_automatic_fillers && g.create_automatic_fillers_angled);
        assert!(!g.resize_by_grid && !g.show_closed_doors_drawers);
    }
}
